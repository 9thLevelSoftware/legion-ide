use devil_protocol::{
    BufferId, BufferVersion, CanonicalPath, CapabilityCommandClass, CapabilityDecisionId,
    CapabilityId, CapabilityNamespace, CapabilityRequest, CapabilityRequestContext, CausalityId,
    ChangedTextRange, CorrelationId, EventEnvelope, EventId, EventMetadataRecord, EventSequence,
    EventSeverity, FileConflictContext, FileConflictLifecycleState, FileConflictReason,
    FileConflictState, FileContentVersion, FileFingerprint, FileId, FileIdentity, FileKind,
    FileMetadata, LargeFileStatus, LineIndexRange, NetworkTarget, PrincipalId, ProposalAuditRecord,
    ProposalDenialReason, ProposalFailureReason, ProposalLifecycleState,
    ProposalLifecycleTransition, ProposalPayload, ProposalPayloadKind, ProposalPayloadSummary,
    ProposalRejectionReason, ProposalResponse, ProposalRollbackReason, ProposalStaleContext,
    ProposalStaleReason, ProposalVersionPreconditions, ProtocolDiagnostic,
    ProtocolDiagnosticSeverity, ProtocolTextRange, RedactionHint, RetentionLabel,
    SaveConflictPolicy, SaveFileProposal, SaveIntent, SessionDirtyIndicator, SessionLayoutSplit,
    SessionPanelState, SessionSplitOrientation, SessionTab, SessionTabGroup,
    SnapshotChunkDescriptor, SnapshotConsumerKind, SnapshotId, SnapshotLeaseDescriptor,
    StorageRepositoryRequest, StorageRepositoryResponse, TextCoordinate, TextTransactionDescriptor,
    TimestampMillis, TransactionSource, TrustDecisionContext, TrustRecord, Utf16Position,
    Utf16Range, VersionContext, ViewportDimensions, ViewportLineMetric, ViewportLineSlice,
    ViewportLineTruncationState, ViewportProjection, ViewportProjectionMode, ViewportScroll,
    WorkspaceGeneration, WorkspaceId, WorkspaceSessionRecord, WorkspaceTrustState,
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

fn causality_id() -> CausalityId {
    CausalityId(Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap())
}

fn file_identity() -> FileIdentity {
    FileIdentity {
        file_id: FileId(33),
        workspace_id: WorkspaceId(11),
        canonical_path: CanonicalPath("C:/repo/src/main.rs".to_string()),
        content_version: FileContentVersion(44),
        content_hash: Some("sha256:file".to_string()),
    }
}

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "sha256".to_string(),
        value: value.to_string(),
    }
}

fn chunk_hash(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "blake3-devil-text-chunk".to_string(),
        value: value.to_string(),
    }
}

fn diagnostic(code: &str) -> ProtocolDiagnostic {
    ProtocolDiagnostic {
        code: code.to_string(),
        message: format!("diagnostic {code}"),
        severity: ProtocolDiagnosticSeverity::Warning,
        path: Some(CanonicalPath("C:/repo/src/main.rs".to_string())),
        range: Some(protocol_range()),
    }
}

fn protocol_range() -> ProtocolTextRange {
    ProtocolTextRange {
        start: TextCoordinate {
            line: 1,
            character: 2,
            byte_offset: Some(10),
            utf16_offset: Some(8),
        },
        end: TextCoordinate {
            line: 1,
            character: 6,
            byte_offset: Some(14),
            utf16_offset: Some(12),
        },
    }
}

fn preconditions() -> ProposalVersionPreconditions {
    ProposalVersionPreconditions {
        file_version: Some(FileContentVersion(44)),
        buffer_version: Some(BufferVersion(55)),
        snapshot_id: Some(SnapshotId(66)),
        generation: Some(WorkspaceGeneration(77)),
        file_content_version: Some(FileContentVersion(44)),
        workspace_generation: Some(WorkspaceGeneration(77)),
        expected_fingerprint: Some(fingerprint("expected")),
        expected_file_length: Some(1234),
        expected_modified_at: Some(TimestampMillis(9876)),
    }
}

fn version_context() -> VersionContext {
    VersionContext {
        file_version: FileContentVersion(44),
        buffer_version: BufferVersion(55),
        snapshot_id: SnapshotId(66),
        generation: WorkspaceGeneration(77),
        file_content_version: FileContentVersion(44),
        workspace_generation: WorkspaceGeneration(77),
        fingerprint: Some(fingerprint("expected")),
        file_length: Some(1234),
        modified_at: Some(TimestampMillis(9876)),
    }
}

fn conflict_state(state: FileConflictLifecycleState) -> FileConflictState {
    FileConflictState {
        state,
        context: FileConflictContext {
            workspace_id: WorkspaceId(11),
            file_identity: file_identity(),
            buffer_version: BufferVersion(55),
            file_content_version: FileContentVersion(44),
            snapshot_id: SnapshotId(66),
            disk_fingerprint: Some(fingerprint("disk")),
            expected_fingerprint: Some(fingerprint("expected")),
            reason: FileConflictReason::DiskFingerprintChanged,
            diagnostics: vec![diagnostic("conflict")],
        },
        diagnostics: vec![diagnostic("state")],
        schema_version: 1,
    }
}

fn transition(state: ProposalLifecycleState) -> ProposalLifecycleTransition {
    ProposalLifecycleTransition {
        proposal_id: devil_protocol::ProposalId(700),
        lifecycle_state: state,
        timestamp: TimestampMillis(1700),
        principal: PrincipalId("principal-1".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        diagnostics: vec![diagnostic("proposal")],
    }
}

fn payload_summary() -> ProposalPayloadSummary {
    ProposalPayloadSummary {
        kind: ProposalPayloadKind::SaveFile,
        affected_files: vec![FileId(33)],
        title: Some("save main.rs".to_string()),
        byte_count: Some(1234),
    }
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
fn dto_contracts_viewport_projection_golden_and_required_fields() {
    let dto = ViewportProjection {
        workspace_id: WorkspaceId(11),
        buffer_id: BufferId(22),
        file_id: Some(FileId(33)),
        snapshot_id: SnapshotId(66),
        buffer_version: BufferVersion(77),
        visible_range: protocol_range(),
        selections: vec![protocol_range()],
        cursor: TextCoordinate {
            line: 1,
            character: 4,
            byte_offset: Some(12),
            utf16_offset: Some(10),
        },
        scroll: ViewportScroll {
            top_line: 120,
            left_column: 4,
        },
        dimensions: ViewportDimensions {
            width_px: 1280,
            height_px: 720,
        },
        mode: ViewportProjectionMode::DegradedLargeFile,
        line_slices: vec![ViewportLineSlice {
            line_number: 120,
            visible_text: "fn main() {".to_string(),
            byte_range: devil_protocol::ByteRange::new(4096, 4107),
            utf16_range: Utf16Range {
                start: Utf16Position {
                    line: 120,
                    character: 0,
                },
                end: Utf16Position {
                    line: 120,
                    character: 11,
                },
            },
            chunk_hash: chunk_hash("chunk-0"),
            truncation_state: ViewportLineTruncationState::Trailing,
        }],
        line_metrics: vec![ViewportLineMetric {
            byte_length: 8192,
            utf16_length: 8192,
            line_ending_width: 1,
            exact: false,
        }],
        decoration_spans: vec![],
        fold_ranges: vec![],
        semantic_token_overlays: vec![],
        large_file_status: Some(LargeFileStatus {
            threshold_bytes: 5_242_880,
            byte_len: 9_437_184,
            disabled_overlay_reasons: vec![
                "semantic_tokens_deferred".to_string(),
                "fold_ranges_deferred".to_string(),
            ],
            bounded_search_enabled: true,
            message: "Large file mode active; overlays deferred.".to_string(),
        }),
        schema_version: 2,
    };

    let value = serde_json::to_value(&dto).expect("serialize viewport projection");
    let expected = json!({
        "workspace_id": 11,
        "buffer_id": 22,
        "file_id": 33,
        "snapshot_id": 66,
        "buffer_version": 77,
        "visible_range": {
            "start": {"line": 1, "character": 2, "byte_offset": 10, "utf16_offset": 8},
            "end": {"line": 1, "character": 6, "byte_offset": 14, "utf16_offset": 12}
        },
        "selections": [
            {
                "start": {"line": 1, "character": 2, "byte_offset": 10, "utf16_offset": 8},
                "end": {"line": 1, "character": 6, "byte_offset": 14, "utf16_offset": 12}
            }
        ],
        "cursor": {"line": 1, "character": 4, "byte_offset": 12, "utf16_offset": 10},
        "scroll": {"top_line": 120, "left_column": 4},
        "dimensions": {"width_px": 1280, "height_px": 720},
        "mode": "DegradedLargeFile",
        "line_slices": [
            {
                "line_number": 120,
                "visible_text": "fn main() {",
                "byte_range": {"start": 4096, "end": 4107},
                "utf16_range": {
                    "start": {"line": 120, "character": 0},
                    "end": {"line": 120, "character": 11}
                },
                "chunk_hash": {
                    "algorithm": "blake3-devil-text-chunk",
                    "value": "chunk-0"
                },
                "truncation_state": "Trailing"
            }
        ],
        "line_metrics": [
            {
                "byte_length": 8192,
                "utf16_length": 8192,
                "line_ending_width": 1,
                "exact": false
            }
        ],
        "decoration_spans": [],
        "fold_ranges": [],
        "semantic_token_overlays": [],
        "large_file_status": {
            "threshold_bytes": 5242880,
            "byte_len": 9437184,
            "disabled_overlay_reasons": [
                "semantic_tokens_deferred",
                "fold_ranges_deferred"
            ],
            "bounded_search_enabled": true,
            "message": "Large file mode active; overlays deferred."
        },
        "schema_version": 2
    });
    assert_eq!(value, expected);

    let roundtrip: ViewportProjection =
        serde_json::from_value(value.clone()).expect("deserialize viewport projection");
    assert_eq!(roundtrip.schema_version, 2);
    assert_eq!(roundtrip.line_slices.len(), 1);
    assert!(matches!(
        roundtrip.mode,
        ViewportProjectionMode::DegradedLargeFile
    ));
    assert_eq!(
        roundtrip
            .large_file_status
            .expect("large file status")
            .byte_len,
        9_437_184
    );

    let mut legacy = value.clone();
    let legacy_map = legacy
        .as_object_mut()
        .expect("legacy viewport payload must be JSON object");
    legacy_map.remove("mode");
    legacy_map.remove("line_slices");
    legacy_map.remove("line_metrics");
    legacy_map.remove("decoration_spans");
    legacy_map.remove("fold_ranges");
    legacy_map.remove("semantic_token_overlays");
    legacy_map.remove("large_file_status");
    let legacy_roundtrip: ViewportProjection =
        serde_json::from_value(legacy).expect("deserialize legacy viewport projection");
    assert!(matches!(
        legacy_roundtrip.mode,
        ViewportProjectionMode::Normal
    ));
    assert!(legacy_roundtrip.line_slices.is_empty());
    assert!(legacy_roundtrip.line_metrics.is_empty());
    assert!(legacy_roundtrip.large_file_status.is_none());

    let mut missing_workspace = value.clone();
    remove_required_field::<ViewportProjection>(&mut missing_workspace, "workspace_id");

    let mut missing_schema = value;
    remove_required_field::<ViewportProjection>(&mut missing_schema, "schema_version");
}

#[test]
fn dto_contracts_snapshot_chunk_descriptor_golden_and_required_fields() {
    let dto = SnapshotChunkDescriptor {
        snapshot_id: SnapshotId(66),
        chunk_index: 7,
        byte_range: devil_protocol::ByteRange::new(4096, 8192),
        line_range: LineIndexRange {
            start: 120,
            end: 144,
        },
        byte_len: 4096,
        chunk_hash: chunk_hash("chunk-7"),
        schema_version: 1,
    };

    let value = serde_json::to_value(&dto).expect("serialize snapshot chunk descriptor");
    let expected = json!({
        "snapshot_id": 66,
        "chunk_index": 7,
        "byte_range": {"start": 4096, "end": 8192},
        "line_range": {"start": 120, "end": 144},
        "byte_len": 4096,
        "chunk_hash": {
            "algorithm": "blake3-devil-text-chunk",
            "value": "chunk-7"
        },
        "schema_version": 1
    });
    assert_eq!(value, expected);

    let roundtrip: SnapshotChunkDescriptor =
        serde_json::from_value(value.clone()).expect("deserialize snapshot chunk descriptor");
    assert_eq!(roundtrip.chunk_index, 7);
    assert_eq!(roundtrip.line_range.end, 144);
    assert_eq!(roundtrip.schema_version, 1);

    let mut missing_snapshot = value.clone();
    remove_required_field::<SnapshotChunkDescriptor>(&mut missing_snapshot, "snapshot_id");

    let mut missing_schema = value;
    remove_required_field::<SnapshotChunkDescriptor>(&mut missing_schema, "schema_version");
}

#[test]
fn dto_contracts_snapshot_lease_descriptor_golden_and_required_fields() {
    let lease_id = Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap();
    let dto = SnapshotLeaseDescriptor {
        lease_id,
        snapshot_id: SnapshotId(66),
        consumer_kind: SnapshotConsumerKind::Ui,
        expires_at: TimestampMillis(12_345),
        chunk_count: 2,
        schema_version: 1,
    };

    let value = serde_json::to_value(&dto).expect("serialize snapshot lease descriptor");
    let expected = json!({
        "lease_id": "44444444-4444-4444-4444-444444444444",
        "snapshot_id": 66,
        "consumer_kind": "UI",
        "expires_at": 12345,
        "chunk_count": 2,
        "schema_version": 1
    });
    assert_eq!(value, expected);

    let roundtrip: SnapshotLeaseDescriptor =
        serde_json::from_value(value.clone()).expect("deserialize snapshot lease descriptor");
    assert_eq!(roundtrip.lease_id, lease_id);
    assert!(matches!(roundtrip.consumer_kind, SnapshotConsumerKind::Ui));
    assert_eq!(roundtrip.schema_version, 1);

    let mut missing_lease = value.clone();
    remove_required_field::<SnapshotLeaseDescriptor>(&mut missing_lease, "lease_id");

    let mut missing_schema = value;
    remove_required_field::<SnapshotLeaseDescriptor>(&mut missing_schema, "schema_version");
}

#[test]
fn dto_contracts_save_proposal_golden_with_all_preconditions() {
    let proposal = SaveFileProposal {
        file: file_identity(),
        buffer_id: BufferId(22),
        file_id: FileId(33),
        snapshot_id: SnapshotId(66),
        buffer_version: BufferVersion(55),
        file_content_version: FileContentVersion(44),
        workspace_generation: WorkspaceGeneration(77),
        expected_fingerprint: Some(fingerprint("expected")),
        save_intent: SaveIntent::Manual,
        conflict_policy: SaveConflictPolicy::RejectIfChanged,
        trust_decision: TrustDecisionContext {
            workspace_trust_state: WorkspaceTrustState::Trusted,
            decision_id: Some(CapabilityDecisionId(88)),
            decided_at: Some(TimestampMillis(999)),
        },
        required_capability: CapabilityId("fs.write".to_string()),
        principal: PrincipalId("principal-1".to_string()),
        correlation_id: CorrelationId(901),
        diagnostics: vec![diagnostic("save")],
    };

    let envelope = ProposalPayload::SaveFile(proposal);
    let value = serde_json::to_value(&envelope).expect("serialize save proposal");
    let expected = json!({
        "SaveFile": {
            "file": {
                "file_id": 33,
                "workspace_id": 11,
                "canonical_path": "C:/repo/src/main.rs",
                "content_version": 44,
                "content_hash": "sha256:file"
            },
            "buffer_id": 22,
            "file_id": 33,
            "snapshot_id": 66,
            "buffer_version": 55,
            "file_content_version": 44,
            "workspace_generation": 77,
            "expected_fingerprint": {"algorithm": "sha256", "value": "expected"},
            "save_intent": "Manual",
            "conflict_policy": "RejectIfChanged",
            "trust_decision": {
                "workspace_trust_state": "Trusted",
                "decision_id": 88,
                "decided_at": 999
            },
            "required_capability": "fs.write",
            "principal": "principal-1",
            "correlation_id": 901,
            "diagnostics": [
                {
                    "code": "save",
                    "message": "diagnostic save",
                    "severity": "Warning",
                    "path": "C:/repo/src/main.rs",
                    "range": {
                        "start": {"line": 1, "character": 2, "byte_offset": 10, "utf16_offset": 8},
                        "end": {"line": 1, "character": 6, "byte_offset": 14, "utf16_offset": 12}
                    }
                }
            ]
        }
    });
    assert_eq!(value, expected);

    let preconditions = preconditions();
    let precondition_value = serde_json::to_value(&preconditions).expect("serialize preconditions");
    assert_eq!(
        precondition_value,
        json!({
            "file_version": 44,
            "buffer_version": 55,
            "snapshot_id": 66,
            "generation": 77,
            "file_content_version": 44,
            "workspace_generation": 77,
            "expected_fingerprint": {"algorithm": "sha256", "value": "expected"},
            "expected_file_length": 1234,
            "expected_modified_at": 9876
        })
    );
    assert!(!preconditions.is_stale(version_context()));

    let mut stale_context = version_context();
    stale_context.fingerprint = Some(fingerprint("disk"));
    assert!(preconditions.is_stale(stale_context));
}

#[test]
fn dto_contracts_each_conflict_state_golden() {
    let states = vec![
        FileConflictLifecycleState::Clean,
        FileConflictLifecycleState::Dirty,
        FileConflictLifecycleState::Saving,
        FileConflictLifecycleState::SaveFailed,
        FileConflictLifecycleState::DiskChangedClean,
        FileConflictLifecycleState::ConflictDirty,
        FileConflictLifecycleState::ReloadAvailable,
        FileConflictLifecycleState::KeepBothPending,
        FileConflictLifecycleState::ComparePending,
    ];

    let value = serde_json::to_value(
        states
            .into_iter()
            .map(conflict_state)
            .collect::<Vec<FileConflictState>>(),
    )
    .expect("serialize conflict states");

    let expected_states = [
        "Clean",
        "Dirty",
        "Saving",
        "SaveFailed",
        "DiskChangedClean",
        "ConflictDirty",
        "ReloadAvailable",
        "KeepBothPending",
        "ComparePending",
    ];
    assert_eq!(value.as_array().expect("conflict array").len(), 9);
    for (item, expected_state) in value
        .as_array()
        .expect("conflict array")
        .iter()
        .zip(expected_states)
    {
        assert_eq!(item["state"], expected_state);
        assert_eq!(item["context"]["workspace_id"], 11);
        assert_eq!(item["context"]["file_identity"]["file_id"], 33);
        assert_eq!(item["context"]["buffer_version"], 55);
        assert_eq!(item["context"]["file_content_version"], 44);
        assert_eq!(item["context"]["snapshot_id"], 66);
        assert_eq!(item["context"]["disk_fingerprint"]["value"], "disk");
        assert_eq!(item["context"]["expected_fingerprint"]["value"], "expected");
        assert_eq!(item["context"]["reason"], "DiskFingerprintChanged");
        assert_eq!(item["schema_version"], 1);
    }

    let roundtrip: Vec<FileConflictState> =
        serde_json::from_value(value).expect("deserialize conflict states");
    assert!(matches!(
        roundtrip[5].state,
        FileConflictLifecycleState::ConflictDirty
    ));
}

#[test]
fn dto_contracts_each_proposal_lifecycle_response_golden() {
    let proposal = Box::new(devil_protocol::WorkspaceProposal {
        proposal_id: devil_protocol::ProposalId(700),
        principal: PrincipalId("principal-1".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(901),
        payload: ProposalPayload::SaveFile(SaveFileProposal {
            file: file_identity(),
            buffer_id: BufferId(22),
            file_id: FileId(33),
            snapshot_id: SnapshotId(66),
            buffer_version: BufferVersion(55),
            file_content_version: FileContentVersion(44),
            workspace_generation: WorkspaceGeneration(77),
            expected_fingerprint: Some(fingerprint("expected")),
            save_intent: SaveIntent::Manual,
            conflict_policy: SaveConflictPolicy::RejectIfChanged,
            trust_decision: TrustDecisionContext {
                workspace_trust_state: WorkspaceTrustState::Trusted,
                decision_id: Some(CapabilityDecisionId(88)),
                decided_at: Some(TimestampMillis(999)),
            },
            required_capability: CapabilityId("fs.write".to_string()),
            principal: PrincipalId("principal-1".to_string()),
            correlation_id: CorrelationId(901),
            diagnostics: vec![],
        }),
        preconditions: preconditions(),
        preview: devil_protocol::PreviewSummary {
            summary: "save".to_string(),
            details: vec!["one file".to_string()],
        },
        expires_at: Some(TimestampMillis(2000)),
        created_at: TimestampMillis(1000),
    });

    let responses = vec![
        ProposalResponse::Created(transition(ProposalLifecycleState::Created)),
        ProposalResponse::Validated(transition(ProposalLifecycleState::Validated)),
        ProposalResponse::Previewed {
            transition: transition(ProposalLifecycleState::Previewed),
            proposal: proposal.clone(),
        },
        ProposalResponse::Approved(transition(ProposalLifecycleState::Approved)),
        ProposalResponse::Rejected {
            transition: transition(ProposalLifecycleState::Rejected),
            reason: ProposalRejectionReason::UserRejected,
        },
        ProposalResponse::Applied(transition(ProposalLifecycleState::Applied)),
        ProposalResponse::Denied {
            transition: transition(ProposalLifecycleState::Denied),
            reason: ProposalDenialReason::CapabilityDenied,
        },
        ProposalResponse::Failed {
            transition: transition(ProposalLifecycleState::Failed),
            reason: ProposalFailureReason::ApplyFailed,
        },
        ProposalResponse::RolledBack {
            transition: transition(ProposalLifecycleState::RolledBack),
            reason: ProposalRollbackReason::ApplyFailed,
        },
        ProposalResponse::Stale {
            transition: transition(ProposalLifecycleState::Stale),
            stale: ProposalStaleContext {
                reason: ProposalStaleReason::FingerprintMismatch,
                expected: preconditions(),
                actual: Some(version_context()),
            },
        },
        ProposalResponse::Conflict {
            transition: transition(ProposalLifecycleState::Conflict),
            conflict: conflict_state(FileConflictLifecycleState::ConflictDirty),
        },
    ];

    let value = serde_json::to_value(&responses).expect("serialize lifecycle responses");
    let expected_variants = [
        "Created",
        "Validated",
        "Previewed",
        "Approved",
        "Rejected",
        "Applied",
        "Denied",
        "Failed",
        "RolledBack",
        "Stale",
        "Conflict",
    ];

    for (item, expected_variant) in value
        .as_array()
        .expect("responses")
        .iter()
        .zip(expected_variants)
    {
        let object = item.as_object().expect("response variant object");
        assert_eq!(object.keys().next().expect("variant key"), expected_variant);
    }
    assert_eq!(value[0]["Created"]["lifecycle_state"], "Created");
    assert_eq!(value[2]["Previewed"]["proposal"]["proposal_id"], 700);
    assert_eq!(value[4]["Rejected"]["reason"], "UserRejected");
    assert_eq!(value[6]["Denied"]["reason"], "CapabilityDenied");
    assert_eq!(value[9]["Stale"]["stale"]["reason"], "FingerprintMismatch");
    assert_eq!(value[10]["Conflict"]["conflict"]["state"], "ConflictDirty");

    let roundtrip: Vec<ProposalResponse> =
        serde_json::from_value(value).expect("deserialize lifecycle responses");
    assert!(matches!(roundtrip[0], ProposalResponse::Created(_)));
    assert!(matches!(roundtrip[10], ProposalResponse::Conflict { .. }));
}

#[test]
fn dto_contracts_session_record_schema_golden() {
    let session = WorkspaceSessionRecord {
        session_id: "session-1".to_string(),
        last_workspace: Some(WorkspaceId(11)),
        last_workspace_path: Some(CanonicalPath("C:/repo".to_string())),
        open_tabs: vec![SessionTab {
            tab_id: "tab-1".to_string(),
            buffer_id: Some(BufferId(22)),
            file_id: Some(FileId(33)),
            path: Some(CanonicalPath("C:/repo/src/main.rs".to_string())),
            title: "main.rs".to_string(),
            pinned: true,
            preview: false,
            dirty: true,
        }],
        active_tab: Some("tab-1".to_string()),
        active_buffer: Some(BufferId(22)),
        tab_groups: vec![SessionTabGroup {
            group_id: "group-1".to_string(),
            tab_ids: vec!["tab-1".to_string()],
            active_tab_id: Some("tab-1".to_string()),
        }],
        layout_splits: vec![SessionLayoutSplit {
            split_id: "split-1".to_string(),
            orientation: SessionSplitOrientation::Horizontal,
            first: "group-1".to_string(),
            second: "group-2".to_string(),
            ratio: 0.5,
        }],
        explorer_expansion: vec![CanonicalPath("C:/repo/src".to_string())],
        panel_state: SessionPanelState {
            bottom_visible: true,
            side_visible: true,
            active_panel: Some("terminal".to_string()),
            bottom_height_px: Some(240),
            side_width_px: Some(320),
        },
        dirty_indicators: vec![SessionDirtyIndicator {
            buffer_id: BufferId(22),
            file_id: Some(FileId(33)),
            dirty: true,
            buffer_version: BufferVersion(55),
        }],
        saved_at: TimestampMillis(5000),
        schema_version: 1,
    };

    let value = serde_json::to_value(&session).expect("serialize session");
    let expected = json!({
        "session_id": "session-1",
        "last_workspace": 11,
        "last_workspace_path": "C:/repo",
        "open_tabs": [{
            "tab_id": "tab-1",
            "buffer_id": 22,
            "file_id": 33,
            "path": "C:/repo/src/main.rs",
            "title": "main.rs",
            "pinned": true,
            "preview": false,
            "dirty": true
        }],
        "active_tab": "tab-1",
        "active_buffer": 22,
        "tab_groups": [{
            "group_id": "group-1",
            "tab_ids": ["tab-1"],
            "active_tab_id": "tab-1"
        }],
        "layout_splits": [{
            "split_id": "split-1",
            "orientation": "Horizontal",
            "first": "group-1",
            "second": "group-2",
            "ratio": 0.5
        }],
        "explorer_expansion": ["C:/repo/src"],
        "panel_state": {
            "bottom_visible": true,
            "side_visible": true,
            "active_panel": "terminal",
            "bottom_height_px": 240,
            "side_width_px": 320
        },
        "dirty_indicators": [{
            "buffer_id": 22,
            "file_id": 33,
            "dirty": true,
            "buffer_version": 55
        }],
        "saved_at": 5000,
        "schema_version": 1
    });
    assert_eq!(value, expected);

    let mut missing = value;
    remove_required_field::<WorkspaceSessionRecord>(&mut missing, "schema_version");
}

#[test]
fn dto_contracts_storage_request_response_schema_golden() {
    let audit = ProposalAuditRecord {
        proposal_id: devil_protocol::ProposalId(700),
        lifecycle_state: ProposalLifecycleState::Applied,
        timestamp: TimestampMillis(1700),
        principal: PrincipalId("principal-1".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        payload_summary: payload_summary(),
        diagnostics: vec![diagnostic("audit")],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let event_id = EventId(Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap());
    let event_metadata = EventMetadataRecord {
        event_id,
        parent_event_id: None,
        causality_id: causality_id(),
        correlation_id: CorrelationId(901),
        event: "proposal.applied".to_string(),
        workspace_id: Some(WorkspaceId(11)),
        sequence: EventSequence(3),
        principal_id: Some(PrincipalId("principal-1".to_string())),
        retention: RetentionLabel::Audit,
        redaction: RedactionHint::MetadataOnly,
        occurred_at: TimestampMillis(1700),
        schema_version: 1,
    };
    let trust = TrustRecord {
        workspace_id: WorkspaceId(11),
        principal_id: PrincipalId("principal-1".to_string()),
        trust_state: WorkspaceTrustState::Trusted,
        decision_id: Some(CapabilityDecisionId(88)),
        correlation_id: CorrelationId(901),
        recorded_at: TimestampMillis(1600),
        schema_version: 1,
    };
    let metadata = FileMetadata {
        canonical_path: CanonicalPath("C:/repo/src/main.rs".to_string()),
        file_id: Some(FileId(33)),
        workspace_id: Some(WorkspaceId(11)),
        kind: FileKind::File,
        size_bytes: Some(1234),
        modified_at: Some(TimestampMillis(9876)),
        read_only: false,
        permissions: Some("rw".to_string()),
        hash: Some("sha256:file".to_string()),
        fingerprint: Some(fingerprint("file")),
        content_version: Some(FileContentVersion(44)),
        workspace_generation: Some(WorkspaceGeneration(77)),
        schema_version: 1,
    };

    let requests = vec![
        StorageRepositoryRequest::SaveFileMetadata(metadata.clone()),
        StorageRepositoryRequest::SaveTrustRecord(trust.clone()),
        StorageRepositoryRequest::SaveProposalAuditRecord(audit.clone()),
        StorageRepositoryRequest::SaveEventMetadata(event_metadata.clone()),
        StorageRepositoryRequest::ReadSessionRecord {
            session_id: "session-1".to_string(),
        },
        StorageRepositoryRequest::ReadTrustRecord {
            workspace_id: WorkspaceId(11),
            principal_id: PrincipalId("principal-1".to_string()),
        },
        StorageRepositoryRequest::ReadProposalAuditRecord(devil_protocol::ProposalId(700)),
        StorageRepositoryRequest::ReadEventMetadata(event_id),
    ];
    let request_value = serde_json::to_value(&requests).expect("serialize storage requests");
    assert_eq!(
        request_value[0]["SaveFileMetadata"]["canonical_path"],
        "C:/repo/src/main.rs"
    );
    assert_eq!(request_value[1]["SaveTrustRecord"]["workspace_id"], 11);
    assert_eq!(
        request_value[2]["SaveProposalAuditRecord"]["proposal_id"],
        700
    );
    assert_eq!(
        request_value[3]["SaveEventMetadata"]["event"],
        "proposal.applied"
    );
    assert_eq!(
        request_value[4]["ReadSessionRecord"]["session_id"],
        "session-1"
    );
    assert_eq!(request_value[6]["ReadProposalAuditRecord"], 700);

    let responses = vec![
        StorageRepositoryResponse::FileMetadata(Some(metadata)),
        StorageRepositoryResponse::TrustRecord(Some(trust)),
        StorageRepositoryResponse::ProposalAuditRecord(Some(audit)),
        StorageRepositoryResponse::EventMetadata(Some(event_metadata)),
    ];
    let response_value = serde_json::to_value(&responses).expect("serialize storage responses");
    assert_eq!(
        response_value[0]["FileMetadata"]["canonical_path"],
        "C:/repo/src/main.rs"
    );
    assert_eq!(
        response_value[1]["TrustRecord"]["principal_id"],
        "principal-1"
    );
    assert_eq!(
        response_value[2]["ProposalAuditRecord"]["lifecycle_state"],
        "Applied"
    );
    assert_eq!(response_value[3]["EventMetadata"]["correlation_id"], 901);

    let roundtrip: Vec<StorageRepositoryRequest> =
        serde_json::from_value(request_value).expect("deserialize storage requests");
    assert!(matches!(
        roundtrip[6],
        StorageRepositoryRequest::ReadProposalAuditRecord(_)
    ));
}

#[test]
fn dto_contracts_event_envelope_golden_and_required_fields() {
    let event_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
    let parent_event_id = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
    let causality_uuid = Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap();

    let envelope = EventEnvelope {
        schema_version: 1,
        event_id: EventId(event_id),
        parent_event_id: Some(EventId(parent_event_id)),
        causality_id: CausalityId(causality_uuid),
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

    assert_ne!(envelope.correlation_id, CorrelationId(0));
    assert_ne!(envelope.causality_id.0, Uuid::nil());

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

#[test]
fn dto_contracts_capability_request_context_golden_and_required_fields() {
    let request = CapabilityRequest::Request {
        principal_id: PrincipalId("principal-1".to_string()),
        capability_id: CapabilityId("fs.write".to_string()),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        target_path: Some(CanonicalPath("C:/repo/src/main.rs".to_string())),
        decision_id: Some(CapabilityDecisionId(4)),
        context: CapabilityRequestContext {
            write_byte_count: Some(4096),
            command_binary: Some("C:/Windows/System32/cmd.exe".to_string()),
            command_class: Some(CapabilityCommandClass::Terminal),
            network_target: Some(NetworkTarget {
                scheme: "https".to_string(),
                host: "example.test".to_string(),
                port: Some(443),
            }),
            plugin_namespace: Some(CapabilityNamespace("plugins.rust".to_string())),
            lsp_server_binary: Some("rust-analyzer".to_string()),
        },
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
            "context": {
                "write_byte_count": 4096,
                "command_binary": "C:/Windows/System32/cmd.exe",
                "command_class": "Terminal",
                "network_target": {
                    "scheme": "https",
                    "host": "example.test",
                    "port": 443
                },
                "plugin_namespace": "plugins.rust",
                "lsp_server_binary": "rust-analyzer"
            },
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
            context,
            ..
        } => {
            assert!(matches!(
                workspace_trust_state,
                WorkspaceTrustState::Trusted
            ));
            assert_eq!(target_path.expect("target path").0, "C:/repo/src/main.rs");
            assert_eq!(decision_id, Some(CapabilityDecisionId(4)));
            assert_eq!(context.write_byte_count, Some(4096));
            assert!(matches!(
                context.command_class,
                Some(CapabilityCommandClass::Terminal)
            ));
            assert_eq!(
                context.network_target.expect("network target").host,
                "example.test"
            );
            assert_eq!(
                context.plugin_namespace.expect("plugin namespace").0,
                "plugins.rust"
            );
            assert_eq!(
                context.lsp_server_binary.expect("lsp binary"),
                "rust-analyzer"
            );
        }
        _ => panic!("unexpected capability request variant"),
    }

    let mut root = value;
    remove_required_field_in_request_variant(&mut root, "workspace_trust_state");
}

#[test]
fn dto_contracts_text_coordinate_and_viewport_projection_golden() {
    let projection = ViewportProjection {
        workspace_id: WorkspaceId(11),
        buffer_id: BufferId(22),
        file_id: Some(FileId(33)),
        snapshot_id: SnapshotId(66),
        buffer_version: BufferVersion(55),
        visible_range: protocol_range(),
        selections: vec![protocol_range()],
        cursor: TextCoordinate {
            line: 2,
            character: 4,
            byte_offset: Some(20),
            utf16_offset: Some(18),
        },
        scroll: ViewportScroll {
            top_line: 1,
            left_column: 0,
        },
        dimensions: ViewportDimensions {
            width_px: 1280,
            height_px: 720,
        },
        mode: ViewportProjectionMode::Normal,
        line_slices: vec![],
        line_metrics: vec![],
        decoration_spans: vec![],
        fold_ranges: vec![],
        semantic_token_overlays: vec![],
        large_file_status: None,
        schema_version: 2,
    };

    let value = serde_json::to_value(&projection).expect("serialize viewport projection");
    let expected = json!({
        "workspace_id": 11,
        "buffer_id": 22,
        "file_id": 33,
        "snapshot_id": 66,
        "buffer_version": 55,
        "visible_range": {
            "start": {"line": 1, "character": 2, "byte_offset": 10, "utf16_offset": 8},
            "end": {"line": 1, "character": 6, "byte_offset": 14, "utf16_offset": 12}
        },
        "selections": [{
            "start": {"line": 1, "character": 2, "byte_offset": 10, "utf16_offset": 8},
            "end": {"line": 1, "character": 6, "byte_offset": 14, "utf16_offset": 12}
        }],
        "cursor": {"line": 2, "character": 4, "byte_offset": 20, "utf16_offset": 18},
        "scroll": {"top_line": 1, "left_column": 0},
        "dimensions": {"width_px": 1280, "height_px": 720},
        "mode": "Normal",
        "line_slices": [],
        "line_metrics": [],
        "decoration_spans": [],
        "fold_ranges": [],
        "semantic_token_overlays": [],
        "large_file_status": null,
        "schema_version": 2
    });
    assert_eq!(value, expected);

    let roundtrip: ViewportProjection =
        serde_json::from_value(value).expect("deserialize viewport projection");
    assert_eq!(roundtrip.cursor.line, 2);
    assert!(matches!(roundtrip.mode, ViewportProjectionMode::Normal));
}
