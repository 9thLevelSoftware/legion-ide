use std::hash::{Hash, Hasher};

use legion_ai::InlinePredictionRequest;
use legion_protocol::{
    AssistedAiOperationClass, AssistedAiProviderClass, AssistedAiProviderInvocationState, BufferId,
    BufferVersion, CancellationTokenId, CapabilityId, CausalityId, CorrelationId, EventSequence,
    FileContentVersion, FileFingerprint, FileId, InlinePredictionFingerprintMetadata,
    InlinePredictionLatencyMetadata, InlinePredictionProviderMetadata, InlinePredictionRequestId,
    InlinePredictionRequestMetadata, InlinePredictionTriggerKind, LanguageId, RedactionHint,
    SnapshotId, TextCoordinate, TimestampMillis, WorkspaceGeneration, WorkspaceId,
    WorkspaceTrustState,
};

fn inline_prediction_request_fixture(provider_id: &str) -> InlinePredictionRequest {
    InlinePredictionRequest {
        provider: provider_id.to_string(),
        model: "inline-test".to_string(),
        metadata: InlinePredictionRequestMetadata {
            request_id: InlinePredictionRequestId(format!("inline:req:{provider_id}")),
            workspace_id: WorkspaceId(11),
            buffer_id: BufferId(22),
            file_id: Some(FileId(33)),
            language_id: LanguageId("rust".to_string()),
            cursor: TextCoordinate {
                line: 3,
                character: 4,
                byte_offset: Some(80),
                utf16_offset: Some(80),
            },
            selection: None,
            visible_range: None,
            trigger: InlinePredictionTriggerKind::Automatic,
            fingerprint: InlinePredictionFingerprintMetadata {
                snapshot_id: SnapshotId(66),
                buffer_version: BufferVersion(55),
                file_content_version: Some(FileContentVersion(44)),
                workspace_generation: WorkspaceGeneration(77),
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
            provider: InlinePredictionProviderMetadata {
                provider_id: provider_id.to_string(),
                model_label: "inline-test".to_string(),
                provider_class: AssistedAiProviderClass::Local,
                operation_class: AssistedAiOperationClass::InlinePrediction,
                invocation_state: AssistedAiProviderInvocationState::Planned,
                latency: InlinePredictionLatencyMetadata {
                    queued_ms: 0,
                    inference_ms: 0,
                    total_ms: 0,
                    timed_out: false,
                },
                health_labels: vec!["test".to_string()],
                cost_labels: vec!["local".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            max_prediction_bytes: 256,
            timeout_ms: 100,
            requested_at: TimestampMillis(2000),
            cancellation_token: CancellationTokenId(
                "55555555-5555-5555-5555-555555555555".parse().unwrap(),
            ),
            required_capability: CapabilityId("ai.inline_prediction.invoke".to_string()),
            principal_id: legion_protocol::PrincipalId("principal".to_string()),
            workspace_trust_state: WorkspaceTrustState::Trusted,
            correlation_id: CorrelationId(7),
            causality_id: CausalityId("66666666-6666-6666-6666-666666666666".parse().unwrap()),
            event_sequence: EventSequence(3),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
    }
}

fn stable_hash(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn prompt_serialization_is_deterministic_for_fixed_inputs() {
    let request = inline_prediction_request_fixture("deterministic-local");

    let first = serde_json::to_string_pretty(&request).expect("serialize request");
    let second = serde_json::to_string_pretty(&request).expect("serialize request again");
    let first_value: serde_json::Value = serde_json::from_str(&first).expect("parse first json");
    let second_value: serde_json::Value = serde_json::from_str(&second).expect("parse second json");

    assert_eq!(first, second);
    assert_eq!(first_value, second_value);
}

#[test]
fn prompt_metadata_hash_is_stable_for_equivalent_requests() {
    let first =
        serde_json::to_string_pretty(&inline_prediction_request_fixture("deterministic-local"))
            .expect("serialize first request");
    let second =
        serde_json::to_string_pretty(&inline_prediction_request_fixture("deterministic-local"))
            .expect("serialize second request");

    assert_eq!(stable_hash(&first), stable_hash(&second));
    assert_eq!(first, second);
}

#[test]
fn prompt_stability_fixture_records_cache_relevant_prefix_fields() {
    let request = inline_prediction_request_fixture("deterministic-local");
    let json = serde_json::to_value(&request).expect("serialize request value");

    assert_eq!(json["provider"], "deterministic-local");
    assert_eq!(json["model"], "inline-test");
    assert_eq!(json["metadata"]["fingerprint"]["schema_version"], 1);
    assert_eq!(json["metadata"]["provider"]["provider_class"], "Local");
    assert_eq!(json["metadata"]["redaction_hints"][0], "MetadataOnly");
}
