use devil_protocol::{
    BufferId, BufferVersion, CanonicalPath, CapabilityDecisionId, CapabilityId, CapabilityRequest,
    CausalityId, ChangedTextRange, CorrelationId, EventEnvelope, EventId, EventSequence,
    EventSeverity, FileId, PrincipalId, RedactionHint, RetentionLabel, SnapshotId,
    TextTransactionDescriptor, TimestampMillis, TransactionSource, Utf16Position, Utf16Range,
    WorkspaceId, WorkspaceTrustState,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use uuid::Uuid;

fn remove_required_field<T: DeserializeOwned>(value: &mut Value, field: &str) {
    let map = value
        .as_object_mut()
        .expect("golden payload must be JSON object");
    map.remove(field);
    assert!(
        serde_json::from_value::<T>(value.clone()).is_err(),
        "expected missing required field `{field}` to fail deserialization"
    );
}

fn remove_required_field_in_request_variant(value: &mut Value, field: &str) {
    let inner = value
        .as_object_mut()
        .expect("capability request must be an enum object")
        .get_mut("Request")
        .expect("capability request must contain Request payload");
    let map = inner
        .as_object_mut()
        .expect("Request payload must be object");
    map.remove(field);
    assert!(
        serde_json::from_value::<CapabilityRequest>(value.clone()).is_err(),
        "expected missing required field `{field}` in Request payload to fail deserialization"
    );
}

#[test]
fn dto_contracts_text_transaction_descriptor_golden_and_required_fields() {
    let transaction_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let causality_uuid = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
    let parent_transaction_id = Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();

    let dto = TextTransactionDescriptor {
        workspace_id: WorkspaceId(11),
        buffer_id: BufferId(22),
        file_id: FileId(33),
        transaction_id,
        correlation_id: CorrelationId(44),
        source: TransactionSource::User,
        pre_snapshot_id: SnapshotId(55),
        post_snapshot_id: SnapshotId(66),
        pre_buffer_version: BufferVersion(77),
        post_buffer_version: BufferVersion(78),
        changed_ranges: vec![ChangedTextRange {
            byte_range: devil_protocol::ByteRange::new(10, 14),
            utf16_range: Utf16Range {
                start: Utf16Position {
                    line: 1,
                    character: 3,
                },
                end: Utf16Position {
                    line: 1,
                    character: 7,
                },
            },
        }],
        causality_id: CausalityId(causality_uuid),
        parent_transaction_id: Some(parent_transaction_id),
        schema_version: 1,
        undo_group_id: None,
        occurred_at: TimestampMillis(999),
    };

    let value = serde_json::to_value(&dto).expect("serialize transaction descriptor");
    let expected = json!({
        "workspace_id": 11,
        "buffer_id": 22,
        "file_id": 33,
        "transaction_id": "11111111-1111-1111-1111-111111111111",
        "correlation_id": 44,
        "source": "User",
        "pre_snapshot_id": 55,
        "post_snapshot_id": 66,
        "pre_buffer_version": 77,
        "post_buffer_version": 78,
        "changed_ranges": [
            {
                "byte_range": {"start": 10, "end": 14},
                "utf16_range": {
                    "start": {"line": 1, "character": 3},
                    "end": {"line": 1, "character": 7}
                }
            }
        ],
        "causality_id": "22222222-2222-2222-2222-222222222222",
        "parent_transaction_id": "33333333-3333-3333-3333-333333333333",
        "schema_version": 1,
        "undo_group_id": null,
        "occurred_at": 999
    });
    assert_eq!(value, expected);

    let roundtrip: TextTransactionDescriptor =
        serde_json::from_value(value.clone()).expect("deserialize transaction descriptor");
    assert_eq!(roundtrip.schema_version, 1);
    assert_eq!(roundtrip.changed_ranges.len(), 1);
    assert_eq!(roundtrip.causality_id, CausalityId(causality_uuid));

    let mut missing = value;
    remove_required_field::<TextTransactionDescriptor>(&mut missing, "causality_id");
}

#[test]
fn dto_contracts_capability_request_golden_and_required_fields() {
    let request = CapabilityRequest::Request {
        principal_id: PrincipalId("principal-1".to_string()),
        capability_id: CapabilityId("fs.write".to_string()),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        target_path: Some(CanonicalPath("C:/repo/src/main.rs".to_string())),
        decision_id: Some(CapabilityDecisionId(4)),
        correlation_id: CorrelationId(91),
    };

    let value = serde_json::to_value(&request).expect("serialize capability request");
    let expected = json!({
        "Request": {
            "principal_id": "principal-1",
            "capability_id": "fs.write",
            "workspace_trust_state": "Trusted",
            "target_path": "C:/repo/src/main.rs",
            "decision_id": 4,
            "correlation_id": 91
        }
    });
    assert_eq!(value, expected);

    let parsed: CapabilityRequest =
        serde_json::from_value(value.clone()).expect("deserialize capability request");
    match parsed {
        CapabilityRequest::Request {
            workspace_trust_state,
            target_path,
            decision_id,
            ..
        } => {
            assert!(matches!(
                workspace_trust_state,
                WorkspaceTrustState::Trusted
            ));
            assert_eq!(
                target_path.expect("target path").0,
                "C:/repo/src/main.rs".to_string()
            );
            assert_eq!(decision_id, Some(CapabilityDecisionId(4)));
        }
        _ => panic!("unexpected capability request variant"),
    }

    let mut root = value;
    remove_required_field_in_request_variant(&mut root, "workspace_trust_state");
}

#[test]
fn dto_contracts_event_envelope_golden_and_required_fields() {
    let event_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
    let parent_event_id = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
    let causality_id = Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap();

    let envelope = EventEnvelope {
        schema_version: 1,
        event_id: EventId(event_id),
        parent_event_id: Some(EventId(parent_event_id)),
        causality_id: CausalityId(causality_id),
        event: "workspace.save_denied".to_string(),
        severity: EventSeverity::Warning,
        retention: RetentionLabel::Audit,
        redaction: RedactionHint::MetadataOnly,
        correlation_id: CorrelationId(123),
        workspace_id: Some(WorkspaceId(55)),
        sequence: EventSequence(77),
        principal_id: Some(PrincipalId("principal-7".to_string())),
        occurred_at: TimestampMillis(5_000),
        payload: json!({"reason": "untrusted-workspace"}),
    };

    let value = serde_json::to_value(&envelope).expect("serialize event envelope");
    let expected = json!({
        "schema_version": 1,
        "event_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "parent_event_id": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        "causality_id": "cccccccc-cccc-cccc-cccc-cccccccccccc",
        "event": "workspace.save_denied",
        "severity": "Warning",
        "retention": "Audit",
        "redaction": "MetadataOnly",
        "correlation_id": 123,
        "workspace_id": 55,
        "sequence": 77,
        "principal_id": "principal-7",
        "occurred_at": 5000,
        "payload": {"reason": "untrusted-workspace"}
    });
    assert_eq!(value, expected);

    let parsed: EventEnvelope =
        serde_json::from_value(value.clone()).expect("deserialize event envelope");
    assert!(matches!(parsed.severity, EventSeverity::Warning));
    assert!(matches!(parsed.retention, RetentionLabel::Audit));
    assert!(matches!(parsed.redaction, RedactionHint::MetadataOnly));

    let mut missing = value;
    remove_required_field::<EventEnvelope>(&mut missing, "schema_version");
}
