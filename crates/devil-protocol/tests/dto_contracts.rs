use devil_protocol::*;
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

fn collaboration_vector() -> CollaborationVersionVector {
    CollaborationVersionVector {
        entries: vec![CollaborationVersionVectorEntry {
            participant_id: CollaborationParticipantId(2001),
            sequence: 7,
        }],
    }
}

fn collaboration_capability_decision() -> CapabilityDecision {
    CapabilityDecision {
        decision_id: CapabilityDecisionId(901),
        granted: true,
        capability: CapabilityId("collaboration.operation.publish".to_string()),
        reason: None,
    }
}

fn collaboration_preconditions() -> CollaborationOperationPreconditions {
    CollaborationOperationPreconditions {
        workspace_id: WorkspaceId(11),
        file_id: FileId(33),
        buffer_id: BufferId(22),
        snapshot_id: SnapshotId(66),
        buffer_version: BufferVersion(55),
        document_epoch: CollaborationDocumentEpoch(3),
        base_vector: collaboration_vector(),
        author_principal: PrincipalId("principal-1".to_string()),
        capability_decision: collaboration_capability_decision(),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
    }
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

fn trust_reference(
    id: &str,
    kind: AssistedAiTrustProjectionKind,
) -> AssistedAiTrustProjectionReference {
    AssistedAiTrustProjectionReference {
        reference_id: id.to_string(),
        kind,
        projection_hash: fingerprint(id),
        schema_version: 1,
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

fn empty_preconditions() -> ProposalVersionPreconditions {
    ProposalVersionPreconditions {
        file_version: None,
        buffer_version: None,
        snapshot_id: None,
        generation: None,
        file_content_version: None,
        workspace_generation: None,
        expected_fingerprint: None,
        expected_file_length: None,
        expected_modified_at: None,
    }
}

fn batch_target_coverage() -> ProposalTargetCoverage {
    ProposalTargetCoverage {
        coverage_kind: ProposalTargetCoverageKind::Complete,
        targets: vec![
            ProposalAffectedTarget {
                target_id: "target-buffer-main".to_string(),
                kind: ProposalTargetKind::OpenBuffer,
                workspace_id: Some(WorkspaceId(11)),
                file_id: Some(FileId(33)),
                buffer_id: Some(BufferId(22)),
                path: Some(CanonicalPath("C:/repo/src/main.rs".to_string())),
                terminal_session_id: None,
                plugin_id: None,
                remote_authority: None,
                collaboration_session_id: None,
                byte_ranges: vec![devil_protocol::ByteRange::new(10, 14)],
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            ProposalAffectedTarget {
                target_id: "target-file-lib".to_string(),
                kind: ProposalTargetKind::PathOnly,
                workspace_id: Some(WorkspaceId(11)),
                file_id: None,
                buffer_id: None,
                path: Some(CanonicalPath("C:/repo/src/lib.rs".to_string())),
                terminal_session_id: None,
                plugin_id: None,
                remote_authority: None,
                collaboration_session_id: None,
                byte_ranges: vec![],
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
        ],
        omitted_target_count: 0,
        redaction_hints: vec![RedactionHint::MetadataOnly],
    }
}

fn batch_payload() -> BatchProposalPayload {
    BatchProposalPayload {
        batch_id: Uuid::parse_str("55555555-5555-5555-5555-555555555555").unwrap(),
        atomicity: ProposalBatchAtomicity::PrepareAllBeforeMutate,
        rollback_policy: ProposalBatchRollbackPolicy::Required,
        target_coverage: batch_target_coverage(),
        items: vec![
            ProposalBatchItem {
                order: 0,
                item_id: "item-edit-main".to_string(),
                payload: Box::new(ProposalPayload::TextEdit(
                    devil_protocol::TextEditProposal {
                        file_id: FileId(33),
                        edits: devil_protocol::EditBatch {
                            edits: vec![devil_protocol::TextEdit {
                                range: devil_protocol::TextRange::byte(10, 14),
                                replacement: "main".to_string(),
                            }],
                        },
                    },
                )),
                target_ids: vec!["target-buffer-main".to_string()],
                required_capability: CapabilityId("editor.write".to_string()),
                rollback_step_ids: vec!["rollback-edit-main".to_string()],
            },
            ProposalBatchItem {
                order: 1,
                item_id: "item-create-lib".to_string(),
                payload: Box::new(ProposalPayload::CreateFile(CreateFileProposal {
                    path: CanonicalPath("C:/repo/src/lib.rs".to_string()),
                    initial_content: None,
                })),
                target_ids: vec!["target-file-lib".to_string()],
                required_capability: CapabilityId("fs.write".to_string()),
                rollback_step_ids: vec!["rollback-create-lib".to_string()],
            },
        ],
        dependency_edges: vec![ProposalBatchDependency {
            prerequisite_item_id: "item-edit-main".to_string(),
            dependent_item_id: "item-create-lib".to_string(),
            kind: ProposalBatchDependencyKind::RequiresValidation,
        }],
        rollback_steps: vec![
            ProposalRollbackStep {
                order: 0,
                step_id: "rollback-create-lib".to_string(),
                item_id: "item-create-lib".to_string(),
                target_id: "target-file-lib".to_string(),
                action: ProposalRollbackAction::DeleteCreatedFile,
                expected_preconditions: empty_preconditions(),
                diagnostics: vec![],
            },
            ProposalRollbackStep {
                order: 1,
                step_id: "rollback-edit-main".to_string(),
                item_id: "item-edit-main".to_string(),
                target_id: "target-buffer-main".to_string(),
                action: ProposalRollbackAction::EditorUndoGroup,
                expected_preconditions: preconditions(),
                diagnostics: vec![diagnostic("rollback")],
            },
        ],
        partial_failures: vec![ProposalPartialFailureRecord {
            item_id: "item-create-lib".to_string(),
            target_id: "target-file-lib".to_string(),
            reason: ProposalFailureReason::ApplyFailed,
            disposition: ProposalPartialFailureDisposition::RolledBack,
            diagnostics: vec![diagnostic("partial")],
        }],
        preview_warnings: vec![ProposalPreviewWarning {
            code: "proposal.preview.rollback_required".to_string(),
            kind: ProposalPreviewWarningKind::RollbackBestEffort,
            message: "batch includes rollback metadata".to_string(),
            target_id: Some("target-file-lib".to_string()),
            redaction_hints: vec![RedactionHint::MetadataOnly],
        }],
        schema_version: 1,
    }
}

fn lsp_request_id() -> LspRequestId {
    LspRequestId(Uuid::parse_str("12121212-1212-1212-1212-121212121212").unwrap())
}

fn cancellation_token_id() -> CancellationTokenId {
    CancellationTokenId(Uuid::parse_str("34343434-3434-3434-3434-343434343434").unwrap())
}

fn semantic_query_id() -> SemanticQueryId {
    SemanticQueryId(Uuid::parse_str("56565656-5656-5656-5656-565656565656").unwrap())
}

fn lsp_context() -> LspOperationContext {
    LspOperationContext {
        request_id: lsp_request_id(),
        workspace_id: WorkspaceId(11),
        file_id: FileId(33),
        buffer_id: BufferId(22),
        snapshot_id: SnapshotId(66),
        buffer_version: BufferVersion(55),
        language_id: LanguageId("rust".to_string()),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        timeout_ms: 2500,
        cancellation_token: cancellation_token_id(),
        content_hash: Some(fingerprint("content")),
        privacy_scope: SemanticPrivacyScope::Workspace,
        schema_version: 1,
    }
}

fn lsp_metadata(status: LspResultStatus) -> LspResultMetadata {
    LspResultMetadata {
        request_id: lsp_request_id(),
        server_id: LanguageServerId(7),
        snapshot_id: SnapshotId(66),
        buffer_version: BufferVersion(55),
        content_hash: Some(fingerprint("content")),
        status,
        generated_at: TimestampMillis(1234),
        diagnostics: vec![diagnostic("lsp")],
        schema_version: 1,
    }
}

fn lsp_location() -> LspLocation {
    LspLocation {
        workspace_id: WorkspaceId(11),
        file_id: FileId(33),
        path: CanonicalPath("C:/repo/src/main.rs".to_string()),
        range: protocol_range(),
        target_selection_range: Some(protocol_range()),
        symbol_name: Some("main".to_string()),
        symbol_kind: Some("Function".to_string()),
    }
}

fn workspace_edit_payload(source: WorkspaceEditSourceKind) -> WorkspaceEditProposalPayload {
    WorkspaceEditProposalPayload {
        workspace_id: WorkspaceId(11),
        edit_id: Uuid::parse_str("78787878-7878-7878-7878-787878787878").unwrap(),
        title: "rename symbol".to_string(),
        source,
        target_coverage: batch_target_coverage(),
        file_edits: vec![WorkspaceTextEdit {
            file: file_identity(),
            buffer_id: Some(BufferId(22)),
            edits: EditBatch {
                edits: vec![TextEdit {
                    range: TextRange::byte(10, 14),
                    replacement: "renamed".to_string(),
                }],
            },
            preconditions: preconditions(),
        }],
        file_operations: vec![
            WorkspaceFileOperation::Create {
                path: CanonicalPath("C:/repo/src/new.rs".to_string()),
                initial_content_hash: Some(fingerprint("new")),
            },
            WorkspaceFileOperation::Delete {
                file: file_identity(),
            },
            WorkspaceFileOperation::Rename {
                file: file_identity(),
                destination: CanonicalPath("C:/repo/src/main_renamed.rs".to_string()),
            },
        ],
        required_capability: CapabilityId("fs.write".to_string()),
        diagnostics: vec![diagnostic("workspace-edit")],
        schema_version: 1,
    }
}

fn lsp_server_identity() -> LspConfiguredServerIdentity {
    LspConfiguredServerIdentity {
        server_id: LanguageServerId(7),
        workspace_id: WorkspaceId(11),
        root_id: Some(WorkspaceRootId(5)),
        language_id: LanguageId("rust".to_string()),
        display_name: "rust-analyzer".to_string(),
        command_hash: fingerprint("cmd-rust-analyzer"),
        args_hash: Some(fingerprint("args-redacted")),
        env_hash: Some(fingerprint("env-redacted")),
        cwd_hash: Some(fingerprint("cwd-redacted")),
        settings_hash: Some(fingerprint("settings-redacted")),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn lsp_trust_posture(
    trust: WorkspaceTrustState,
    privacy_allowed: bool,
) -> LspWorkspaceTrustPosture {
    LspWorkspaceTrustPosture {
        workspace_id: WorkspaceId(11),
        workspace_trust_state: trust,
        privacy_scope: SemanticPrivacyScope::Workspace,
        privacy_scope_allowed: privacy_allowed,
        required_capability: CapabilityId("process.spawn".to_string()),
        decision_id: Some(CapabilityDecisionId(88)),
        diagnostics: vec![diagnostic("lsp-posture")],
        schema_version: 1,
    }
}

fn lsp_request_correlation() -> LspRequestCorrelation {
    LspRequestCorrelation {
        request_id: lsp_request_id(),
        server_id: LanguageServerId(7),
        workspace_id: WorkspaceId(11),
        file_id: Some(FileId(33)),
        snapshot_id: Some(SnapshotId(66)),
        buffer_version: Some(BufferVersion(55)),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        cancellation_token: Some(cancellation_token_id()),
        privacy_scope: SemanticPrivacyScope::Workspace,
        issued_at: TimestampMillis(1100),
        schema_version: 1,
    }
}

fn lsp_diagnostic_summary() -> LspDiagnosticSummary {
    LspDiagnosticSummary {
        workspace_id: WorkspaceId(11),
        file_id: FileId(33),
        snapshot_id: SnapshotId(66),
        buffer_version: BufferVersion(55),
        content_hash: Some(fingerprint("content")),
        diagnostic_count: 2,
        error_count: 1,
        warning_count: 1,
        information_count: 0,
        hint_count: 0,
        ranges: vec![protocol_range()],
        diagnostic_hashes: vec![fingerprint("diagnostic-code-hash")],
        source_hashes: vec![fingerprint("source-label-hash")],
        freshness: SemanticFreshnessState::Fresh,
        privacy_scope: SemanticPrivacyScope::Workspace,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn lsp_edit_conversion_input(source: WorkspaceEditSourceKind) -> LspEditProposalConversionInput {
    LspEditProposalConversionInput {
        proposal_id: devil_protocol::ProposalId(702),
        principal: PrincipalId("principal-1".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        request: lsp_request_correlation(),
        workspace_edit: workspace_edit_payload(source),
        preconditions: preconditions(),
        lifecycle_state: ProposalLifecycleState::Created,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        preview: PreviewSummary {
            summary: "proposal-mediated LSP edit".to_string(),
            details: vec!["metadata-only preview; full diff redacted".to_string()],
        },
        expires_at: Some(TimestampMillis(2000)),
        created_at: TimestampMillis(1000),
        diagnostics: vec![diagnostic("lsp-edit-conversion")],
        schema_version: 1,
    }
}

fn proposal_for_lsp() -> WorkspaceProposal {
    WorkspaceProposal {
        proposal_id: devil_protocol::ProposalId(701),
        principal: PrincipalId("principal-1".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(901),
        payload: ProposalPayload::WorkspaceEdit(workspace_edit_payload(
            WorkspaceEditSourceKind::LspRename,
        )),
        preconditions: preconditions(),
        preview: PreviewSummary {
            summary: "rename symbol".to_string(),
            details: vec!["proposal-mediated LSP rename".to_string()],
        },
        expires_at: Some(TimestampMillis(2000)),
        created_at: TimestampMillis(1000),
    }
}

fn semantic_invalidation_key() -> SemanticInvalidationKey {
    SemanticInvalidationKey {
        workspace_id: WorkspaceId(11),
        file_id: FileId(33),
        snapshot_id: Some(SnapshotId(66)),
        file_content_version: FileContentVersion(44),
        workspace_generation: WorkspaceGeneration(77),
        content_hash: fingerprint("content"),
        grammar_version: Some(SemanticGrammarVersion("tree-sitter-rust@1".to_string())),
        model_version: Some(SemanticModelVersion("ranker@0".to_string())),
        privacy_scope: SemanticPrivacyScope::Workspace,
        schema_version: 1,
    }
}

fn semantic_freshness(state: SemanticFreshnessState) -> SemanticFreshness {
    SemanticFreshness {
        state,
        key: semantic_invalidation_key(),
        degraded_reasons: vec!["lsp_unavailable".to_string()],
        observed_at: TimestampMillis(2222),
    }
}

fn semantic_provenance(source: SemanticRecordSource) -> SemanticRecordProvenance {
    SemanticRecordProvenance {
        source,
        server_id: Some(LanguageServerId(7)),
        extraction_version: "phase3-contract-v1".to_string(),
        confidence_basis_points: 9000,
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
        buffer_id: BufferId(7),
        snapshot_id: SnapshotId(66),
        buffer_version: BufferVersion(9),
        consumer_kind: SnapshotConsumerKind::Ui,
        expires_at: TimestampMillis(12_345),
        chunk_count: 2,
        schema_version: 2,
    };

    let value = serde_json::to_value(&dto).expect("serialize snapshot lease descriptor");
    let expected = json!({
        "lease_id": "44444444-4444-4444-4444-444444444444",
        "buffer_id": 7,
        "snapshot_id": 66,
        "buffer_version": 9,
        "consumer_kind": "UI",
        "expires_at": 12345,
        "chunk_count": 2,
        "schema_version": 2
    });
    assert_eq!(value, expected);

    let roundtrip: SnapshotLeaseDescriptor =
        serde_json::from_value(value.clone()).expect("deserialize snapshot lease descriptor");
    assert_eq!(roundtrip.lease_id, lease_id);
    assert_eq!(roundtrip.buffer_id, BufferId(7));
    assert_eq!(roundtrip.buffer_version, BufferVersion(9));
    assert!(matches!(roundtrip.consumer_kind, SnapshotConsumerKind::Ui));
    assert_eq!(roundtrip.schema_version, 2);

    let mut missing_lease = value.clone();
    remove_required_field::<SnapshotLeaseDescriptor>(&mut missing_lease, "lease_id");

    let mut missing_buffer = value.clone();
    remove_required_field::<SnapshotLeaseDescriptor>(&mut missing_buffer, "buffer_id");

    let mut missing_version = value.clone();
    remove_required_field::<SnapshotLeaseDescriptor>(&mut missing_version, "buffer_version");

    let mut missing_schema = value;
    remove_required_field::<SnapshotLeaseDescriptor>(&mut missing_schema, "schema_version");
}

#[test]
fn dto_contracts_lsp_phase3_definition_references_and_cancellation_golden() {
    let definition_request = LspRequest::Definition(LspDefinitionRequest {
        context: lsp_context(),
        position: TextCoordinate {
            line: 1,
            character: 4,
            byte_offset: Some(12),
            utf16_offset: Some(10),
        },
        include_declaration: true,
    });
    let references_request = LspRequest::References(LspReferenceRequest {
        context: lsp_context(),
        position: TextCoordinate {
            line: 1,
            character: 4,
            byte_offset: Some(12),
            utf16_offset: Some(10),
        },
        include_declaration: false,
        scope: LspReferenceScope::Workspace,
    });
    let cancel_request = LspRequest::Cancel(LspCancellationRequest {
        request_id: lsp_request_id(),
        cancellation_token: cancellation_token_id(),
        reason: "user cancelled navigation".to_string(),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
    });

    let requests = vec![definition_request, references_request, cancel_request];
    let request_value = serde_json::to_value(&requests).expect("serialize phase3 lsp requests");
    let expected_requests = json!([
        {
            "Definition": {
                "context": {
                    "request_id": "12121212-1212-1212-1212-121212121212",
                    "workspace_id": 11,
                    "file_id": 33,
                    "buffer_id": 22,
                    "snapshot_id": 66,
                    "buffer_version": 55,
                    "language_id": "rust",
                    "correlation_id": 901,
                    "causality_id": "cccccccc-cccc-cccc-cccc-cccccccccccc",
                    "timeout_ms": 2500,
                    "cancellation_token": "34343434-3434-3434-3434-343434343434",
                    "content_hash": {"algorithm": "sha256", "value": "content"},
                    "privacy_scope": "Workspace",
                    "schema_version": 1
                },
                "position": {"line": 1, "character": 4, "byte_offset": 12, "utf16_offset": 10},
                "include_declaration": true
            }
        },
        {
            "References": {
                "context": {
                    "request_id": "12121212-1212-1212-1212-121212121212",
                    "workspace_id": 11,
                    "file_id": 33,
                    "buffer_id": 22,
                    "snapshot_id": 66,
                    "buffer_version": 55,
                    "language_id": "rust",
                    "correlation_id": 901,
                    "causality_id": "cccccccc-cccc-cccc-cccc-cccccccccccc",
                    "timeout_ms": 2500,
                    "cancellation_token": "34343434-3434-3434-3434-343434343434",
                    "content_hash": {"algorithm": "sha256", "value": "content"},
                    "privacy_scope": "Workspace",
                    "schema_version": 1
                },
                "position": {"line": 1, "character": 4, "byte_offset": 12, "utf16_offset": 10},
                "include_declaration": false,
                "scope": "Workspace"
            }
        },
        {
            "Cancel": {
                "request_id": "12121212-1212-1212-1212-121212121212",
                "cancellation_token": "34343434-3434-3434-3434-343434343434",
                "reason": "user cancelled navigation",
                "correlation_id": 901,
                "causality_id": "cccccccc-cccc-cccc-cccc-cccccccccccc"
            }
        }
    ]);
    assert_eq!(request_value, expected_requests);

    let responses = vec![
        LspResponse::Definition(LspDefinitionResponse {
            metadata: lsp_metadata(LspResultStatus::Fresh),
            locations: vec![lsp_location()],
        }),
        LspResponse::References(LspReferenceResponse {
            metadata: lsp_metadata(LspResultStatus::Partial),
            references: vec![lsp_location()],
        }),
        LspResponse::Cancelled(LspCancellationAck {
            request_id: lsp_request_id(),
            cancellation_token: cancellation_token_id(),
            propagated_to_server: true,
            acknowledged_at: TimestampMillis(1300),
        }),
    ];
    let response_value = serde_json::to_value(&responses).expect("serialize phase3 lsp responses");
    assert_eq!(
        response_value[0]["Definition"]["metadata"]["status"],
        "Fresh"
    );
    assert_eq!(
        response_value[0]["Definition"]["locations"][0]["symbol_name"],
        "main"
    );
    assert_eq!(
        response_value[1]["References"]["metadata"]["status"],
        "Partial"
    );
    assert_eq!(response_value[2]["Cancelled"]["propagated_to_server"], true);

    let roundtrip_requests: Vec<LspRequest> =
        serde_json::from_value(request_value).expect("deserialize lsp requests");
    assert!(matches!(roundtrip_requests[0], LspRequest::Definition(_)));
    assert!(matches!(roundtrip_requests[1], LspRequest::References(_)));
    assert!(matches!(roundtrip_requests[2], LspRequest::Cancel(_)));

    let roundtrip_responses: Vec<LspResponse> =
        serde_json::from_value(response_value).expect("deserialize lsp responses");
    assert!(matches!(roundtrip_responses[0], LspResponse::Definition(_)));
    assert!(matches!(roundtrip_responses[1], LspResponse::References(_)));
    assert!(matches!(roundtrip_responses[2], LspResponse::Cancelled(_)));

    let mut missing = serde_json::to_value(lsp_context()).expect("serialize lsp context");
    remove_required_field::<LspOperationContext>(&mut missing, "cancellation_token");
}

#[test]
fn dto_contracts_lsp_phase3_mutation_proposals_are_proposal_ready() {
    let rename_request = LspRequest::Rename(LspRenameRequest {
        context: lsp_context(),
        position: TextCoordinate {
            line: 1,
            character: 4,
            byte_offset: Some(12),
            utf16_offset: Some(10),
        },
        new_name: "renamed".to_string(),
        prepare_only: false,
    });
    let formatting_request = LspRequest::FormattingProposal(LspFormattingProposalRequest {
        context: lsp_context(),
        mode: LspFormattingMode::Document,
        range: None,
        options: LspFormattingOptions {
            tab_size: 4,
            insert_spaces: true,
            trim_trailing_whitespace: true,
            insert_final_newline: true,
            custom_options: vec![("rustfmt.edition".to_string(), "2024".to_string())],
        },
    });
    let code_action_request = LspRequest::CodeActionProposal(LspCodeActionProposalRequest {
        context: lsp_context(),
        range: protocol_range(),
        diagnostics: vec![LspDiagnostic {
            file_id: FileId(33),
            range: TextRange::byte(10, 14),
            severity: LspDiagnosticSeverity::Warning,
            message: "unused import".to_string(),
            source: Some("rust-analyzer".to_string()),
        }],
        only: vec!["quickfix".to_string(), "refactor.rename".to_string()],
    });

    let request_value = serde_json::to_value(vec![
        rename_request,
        formatting_request,
        code_action_request,
    ])
    .expect("serialize mutation lsp requests");
    assert_eq!(request_value[0]["Rename"]["new_name"], "renamed");
    assert_eq!(request_value[1]["FormattingProposal"]["mode"], "Document");
    assert_eq!(
        request_value[1]["FormattingProposal"]["options"]["custom_options"][0][0],
        "rustfmt.edition"
    );
    assert_eq!(
        request_value[2]["CodeActionProposal"]["only"][0],
        "quickfix"
    );

    let rename_response = LspResponse::Rename(LspRenameResponse {
        metadata: lsp_metadata(LspResultStatus::Fresh),
        prepare: Some(LspPrepareRenameResult {
            range: protocol_range(),
            placeholder: Some("main".to_string()),
            allowed: true,
        }),
        proposal: Some(LspMutationProposalResult::Proposal(Box::new(
            proposal_for_lsp(),
        ))),
    });
    let formatting_response = LspResponse::FormattingProposal(LspFormattingProposalResponse {
        metadata: lsp_metadata(LspResultStatus::Fresh),
        proposal: LspMutationProposalResult::NoChanges {
            diagnostics: vec![diagnostic("already-formatted")],
        },
    });
    let code_action_response = LspResponse::CodeActionProposals(LspCodeActionProposalResponse {
        metadata: lsp_metadata(LspResultStatus::Degraded),
        actions: vec![
            LspCodeActionCandidate {
                title: "Apply fix".to_string(),
                kind: Some("quickfix".to_string()),
                is_preferred: true,
                payload: LspCodeActionPayload::EditOnly {
                    workspace_edit: workspace_edit_payload(WorkspaceEditSourceKind::LspCodeAction),
                },
                diagnostics: vec![diagnostic("edit-action")],
            },
            LspCodeActionCandidate {
                title: "Run command".to_string(),
                kind: Some("source.organizeImports".to_string()),
                is_preferred: false,
                payload: LspCodeActionPayload::CommandOnly {
                    command: LspCommandDescriptor {
                        command_id: "rust-analyzer.organizeImports".to_string(),
                        title: "Organize imports".to_string(),
                        argument_hints: vec!["workspace-edit-redacted".to_string()],
                    },
                },
                diagnostics: vec![diagnostic("command-action")],
            },
            LspCodeActionCandidate {
                title: "Edit and command".to_string(),
                kind: Some("refactor.extract".to_string()),
                is_preferred: false,
                payload: LspCodeActionPayload::EditAndCommand {
                    workspace_edit: workspace_edit_payload(WorkspaceEditSourceKind::LspCodeAction),
                    command: LspCommandDescriptor {
                        command_id: "server.followup".to_string(),
                        title: "Follow up".to_string(),
                        argument_hints: vec!["metadata-only".to_string()],
                    },
                },
                diagnostics: vec![],
            },
            LspCodeActionCandidate {
                title: "Disabled".to_string(),
                kind: None,
                is_preferred: false,
                payload: LspCodeActionPayload::Disabled {
                    reason: "server returned unsafe edit".to_string(),
                },
                diagnostics: vec![diagnostic("disabled-action")],
            },
        ],
    });

    let response_value = serde_json::to_value(vec![
        rename_response,
        formatting_response,
        code_action_response,
    ])
    .expect("serialize mutation lsp responses");
    assert_eq!(response_value[0]["Rename"]["prepare"]["allowed"], true);
    assert_eq!(
        response_value[0]["Rename"]["proposal"]["Proposal"]["payload"]["WorkspaceEdit"]["source"],
        "LspRename"
    );
    assert_eq!(
        response_value[1]["FormattingProposal"]["proposal"]["NoChanges"]["diagnostics"][0]["code"],
        "already-formatted"
    );
    assert_eq!(
        response_value[2]["CodeActionProposals"]["actions"][0]["payload"]["EditOnly"]["workspace_edit"]
            ["source"],
        "LspCodeAction"
    );
    assert_eq!(
        response_value[2]["CodeActionProposals"]["actions"][1]["payload"]["CommandOnly"]["command"]
            ["command_id"],
        "rust-analyzer.organizeImports"
    );
    assert_eq!(
        response_value[2]["CodeActionProposals"]["actions"][2]["payload"]["EditAndCommand"]["command"]
            ["title"],
        "Follow up"
    );
    assert_eq!(
        response_value[2]["CodeActionProposals"]["actions"][3]["payload"]["Disabled"]["reason"],
        "server returned unsafe edit"
    );

    let payload_value = serde_json::to_value(ProposalPayload::WorkspaceEdit(
        workspace_edit_payload(WorkspaceEditSourceKind::SemanticRefactor),
    ))
    .expect("serialize workspace edit proposal payload");
    assert_eq!(payload_value["WorkspaceEdit"]["source"], "SemanticRefactor");
    assert_eq!(
        payload_value["WorkspaceEdit"]["file_edits"][0]["preconditions"]["snapshot_id"],
        66
    );
    assert_eq!(
        payload_value["WorkspaceEdit"]["file_operations"][0]["Create"]["path"],
        "C:/repo/src/new.rs"
    );
    assert_eq!(
        payload_value["WorkspaceEdit"]["file_operations"][1]["Delete"]["file"]["file_id"],
        33
    );
    assert_eq!(
        payload_value["WorkspaceEdit"]["file_operations"][2]["Rename"]["destination"],
        "C:/repo/src/main_renamed.rs"
    );

    let roundtrip: Vec<LspResponse> =
        serde_json::from_value(response_value).expect("deserialize mutation responses");
    assert!(matches!(roundtrip[0], LspResponse::Rename(_)));
    assert!(matches!(roundtrip[1], LspResponse::FormattingProposal(_)));
    assert!(matches!(roundtrip[2], LspResponse::CodeActionProposals(_)));
}

#[test]
fn dto_contracts_lsp_supervision_boundary_is_metadata_only_and_redacted() {
    let identity = lsp_server_identity();
    let posture = lsp_trust_posture(WorkspaceTrustState::Trusted, true);
    let decision = LspLaunchPolicyDecision::evaluate(
        identity.clone(),
        posture,
        false,
        CorrelationId(901),
        causality_id(),
        vec![diagnostic("runtime-deferred")],
        1,
    );
    let event = LspSupervisionEvent {
        event_id: EventId(Uuid::parse_str("99999999-9999-9999-9999-999999999999").unwrap()),
        sequence: EventSequence(3),
        kind: LspSupervisionEventKind::LaunchRefused,
        identity,
        lifecycle_state: LspSupervisionLifecycleState::LaunchDeferred,
        health_state: LspHealthState::Unavailable,
        request: Some(lsp_request_correlation()),
        restart_backoff: Some(LspRestartBackoffMetadata {
            restart_attempts: 0,
            max_restart_attempts: 3,
            next_backoff_ms: 0,
            circuit_breaker_open: false,
            last_failure_code: Some("runtime-deferred".to_string()),
            last_failure_hash: Some(fingerprint("failure-redacted")),
            schema_version: 1,
        }),
        capabilities: vec![LspCapabilitySummary {
            capability: "rename".to_string(),
            supported: true,
            dynamic_registration: false,
            option_hash: Some(fingerprint("rename-options")),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        diagnostic_summaries: vec![lsp_diagnostic_summary()],
        diagnostics: vec![diagnostic("supervision-event")],
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let value = serde_json::to_value((&decision, &event)).expect("serialize supervision records");
    assert_eq!(value[0]["disposition"], "RuntimeActivationDeferred");
    assert_eq!(value[0]["process_launch_allowed"], false);
    assert_eq!(
        value[0]["identity"]["command_hash"]["value"],
        "cmd-rust-analyzer"
    );
    assert_eq!(value[1]["kind"], "LaunchRefused");
    assert_eq!(value[1]["redaction_hints"], json!(["MetadataOnly"]));

    let serialized = serde_json::to_string(&value).expect("stringify supervision records");
    assert!(!serialized.contains("fn main"));
    assert!(!serialized.contains("raw source"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("terminal output"));
    assert!(!serialized.contains("command\":"));
    assert!(!serialized.contains("env\":"));

    let roundtrip: (LspLaunchPolicyDecision, LspSupervisionEvent) =
        serde_json::from_value(value).expect("deserialize supervision records");
    assert!(!roundtrip.0.process_launch_allowed);
    assert_eq!(roundtrip.1.sequence, EventSequence(3));
}

#[test]
fn dto_contracts_lsp_supervision_refuses_untrusted_or_privacy_denied_without_launch() {
    let untrusted = LspLaunchPolicyDecision::evaluate(
        lsp_server_identity(),
        lsp_trust_posture(WorkspaceTrustState::Untrusted, true),
        true,
        CorrelationId(901),
        causality_id(),
        vec![],
        1,
    );
    assert_eq!(
        untrusted.disposition,
        LspLaunchDisposition::DisabledUntrustedWorkspace
    );
    assert!(!untrusted.process_launch_allowed);

    let privacy_denied = LspLaunchPolicyDecision::evaluate(
        lsp_server_identity(),
        lsp_trust_posture(WorkspaceTrustState::Trusted, false),
        true,
        CorrelationId(901),
        causality_id(),
        vec![],
        1,
    );
    assert_eq!(
        privacy_denied.disposition,
        LspLaunchDisposition::DisabledPrivacyDenied
    );
    assert!(!privacy_denied.process_launch_allowed);
}

#[test]
fn dto_contracts_lsp_edits_convert_to_workspace_proposals_without_direct_mutation() {
    let proposal = convert_lsp_edit_to_workspace_proposal(lsp_edit_conversion_input(
        WorkspaceEditSourceKind::LspRename,
    ))
    .expect("convert lsp edit into proposal");

    assert_eq!(proposal.correlation_id, CorrelationId(901));
    assert_eq!(proposal.preconditions.snapshot_id, Some(SnapshotId(66)));
    assert_eq!(
        proposal.preconditions.workspace_generation,
        Some(WorkspaceGeneration(77))
    );

    match proposal.payload {
        ProposalPayload::WorkspaceEdit(payload) => {
            assert_eq!(payload.source, WorkspaceEditSourceKind::LspRename);
            assert_eq!(
                payload.file_edits[0].preconditions.buffer_version,
                Some(BufferVersion(55))
            );
            assert_eq!(payload.file_edits[0].edits.edits[0].replacement, "renamed");
        }
        _ => panic!("lsp edit must be proposal-mediated workspace edit"),
    }
}

#[test]
fn dto_contracts_lsp_edit_contract_rejects_missing_correlation_privacy_or_preconditions() {
    let mut zero_correlation = lsp_edit_conversion_input(WorkspaceEditSourceKind::LspFormatting);
    zero_correlation.request.correlation_id = CorrelationId(0);
    assert_eq!(
        validate_lsp_edit_proposal_contract(&zero_correlation),
        Err(LspContractValidationError::ZeroCorrelationId)
    );

    let mut privacy_denied = lsp_edit_conversion_input(WorkspaceEditSourceKind::LspFormatting);
    privacy_denied.request.privacy_scope = SemanticPrivacyScope::MetadataOnly;
    assert_eq!(
        validate_lsp_edit_proposal_contract(&privacy_denied),
        Err(LspContractValidationError::PrivacyDenied)
    );

    let mut missing_precondition =
        lsp_edit_conversion_input(WorkspaceEditSourceKind::LspCodeAction);
    missing_precondition.preconditions.snapshot_id = None;
    assert_eq!(
        validate_lsp_edit_proposal_contract(&missing_precondition),
        Err(LspContractValidationError::MissingPrecondition)
    );

    let mut missing_envelope_fingerprint =
        lsp_edit_conversion_input(WorkspaceEditSourceKind::LspCodeAction);
    missing_envelope_fingerprint
        .preconditions
        .expected_fingerprint = None;
    assert_eq!(
        validate_lsp_edit_proposal_contract(&missing_envelope_fingerprint),
        Err(LspContractValidationError::MissingPrecondition)
    );

    let mut missing_file_edit_fingerprint =
        lsp_edit_conversion_input(WorkspaceEditSourceKind::LspCodeAction);
    missing_file_edit_fingerprint.workspace_edit.file_edits[0]
        .preconditions
        .expected_fingerprint = None;
    assert_eq!(
        validate_lsp_edit_proposal_contract(&missing_file_edit_fingerprint),
        Err(LspContractValidationError::MissingPrecondition)
    );

    let mut incompatible_lifecycle =
        lsp_edit_conversion_input(WorkspaceEditSourceKind::LspCodeAction);
    incompatible_lifecycle.lifecycle_state = ProposalLifecycleState::Applied;
    assert_eq!(
        validate_lsp_edit_proposal_contract(&incompatible_lifecycle),
        Err(LspContractValidationError::IncompatibleProposalLifecycle)
    );
}

#[test]
fn dto_contracts_lsp_diagnostics_and_capabilities_exclude_source_bodies() {
    let summary = lsp_diagnostic_summary();
    let capability = LspCapabilitySummary {
        capability: "code_action".to_string(),
        supported: true,
        dynamic_registration: true,
        option_hash: Some(fingerprint("code-action-options")),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let value = serde_json::to_value((&summary, &capability)).expect("serialize summaries");
    assert_eq!(value[0]["diagnostic_count"], 2);
    assert_eq!(
        value[0]["diagnostic_hashes"][0]["value"],
        "diagnostic-code-hash"
    );
    assert_eq!(value[1]["option_hash"]["value"], "code-action-options");

    let serialized = serde_json::to_string(&value).expect("stringify summaries");
    assert!(!serialized.contains("unused import"));
    assert!(!serialized.contains("fn main"));
    assert!(!serialized.contains("source_body"));
    assert!(!serialized.contains("message"));
}

#[test]
fn dto_contracts_lsp_supervision_slice_has_no_external_runtime_request() {
    let decision = LspLaunchPolicyDecision::evaluate(
        lsp_server_identity(),
        lsp_trust_posture(WorkspaceTrustState::Trusted, true),
        false,
        CorrelationId(901),
        causality_id(),
        vec![],
        1,
    );
    let request = LspRequest::RegisterSupervision(Box::new(decision.clone()));
    let response = LspResponse::SupervisionPlanned(Box::new(decision));
    let serialized = serde_json::to_string(&(request, response)).expect("serialize supervision IO");

    assert!(serialized.contains("RegisterSupervision"));
    assert!(serialized.contains("SupervisionPlanned"));
    assert!(serialized.contains("\"process_launch_allowed\":false"));
    assert!(!serialized.contains("RegisterServer"));
    assert!(!serialized.contains("OpenDocument"));
    assert!(!serialized.contains("std::process"));
    assert!(!serialized.contains("thread::spawn"));
}

#[test]
fn dto_contracts_semantic_indexing_records_golden_and_required_fields() {
    let token = SemanticCancellationToken {
        token_id: cancellation_token_id(),
        workspace_id: WorkspaceId(11),
        file_id: Some(FileId(33)),
        snapshot_id: Some(SnapshotId(66)),
        content_hash: Some(fingerprint("content")),
        workspace_generation: Some(WorkspaceGeneration(77)),
        privacy_scope: SemanticPrivacyScope::MetadataOnly,
        reason: Some(SemanticCancellationReason::SnapshotSuperseded),
        issued_at: TimestampMillis(1000),
        expires_at: Some(TimestampMillis(2000)),
        schema_version: 1,
    };
    let file_identity = SemanticFileFingerprintIdentity {
        workspace_id: WorkspaceId(11),
        file_id: FileId(33),
        canonical_path: CanonicalPath("C:/repo/src/main.rs".to_string()),
        file_content_version: FileContentVersion(44),
        workspace_generation: WorkspaceGeneration(77),
        content_hash: fingerprint("content"),
        disk_fingerprint: Some(fingerprint("disk")),
        byte_len: Some(1234),
        modified_at: Some(TimestampMillis(9876)),
        privacy_scope: SemanticPrivacyScope::Workspace,
        schema_version: 1,
    };
    let symbol_map = SymbolFileMapRecord {
        symbol_id: SemanticSymbolId("symbol:main".to_string()),
        symbol_name_hash: fingerprint("symbol-main"),
        display_name: Some("main".to_string()),
        kind: "Function".to_string(),
        workspace_id: WorkspaceId(11),
        file_id: FileId(33),
        path: CanonicalPath("C:/repo/src/main.rs".to_string()),
        language_id: LanguageId("rust".to_string()),
        declaration_range: Some(protocol_range()),
        reference_ranges: vec![protocol_range()],
        invalidation_key: semantic_invalidation_key(),
        provenance: semantic_provenance(SemanticRecordSource::Lexical),
        schema_version: 1,
    };
    let graph_record = SemanticGraphRecord {
        record_id: SemanticRecordId("record:call:main:println".to_string()),
        kind: SemanticGraphRecordKind::CallEdge,
        workspace_id: WorkspaceId(11),
        source: SemanticGraphEndpoint {
            record_id: Some(SemanticRecordId("record:symbol:main".to_string())),
            symbol_id: Some(SemanticSymbolId("symbol:main".to_string())),
            file_id: Some(FileId(33)),
            range: Some(protocol_range()),
        },
        target: Some(SemanticGraphEndpoint {
            record_id: Some(SemanticRecordId("record:symbol:println".to_string())),
            symbol_id: Some(SemanticSymbolId("symbol:println".to_string())),
            file_id: Some(FileId(33)),
            range: Some(protocol_range()),
        }),
        label: "calls".to_string(),
        properties: vec![SemanticProperty {
            key: "receiver".to_string(),
            value: "metadata-only".to_string(),
            redaction: RedactionHint::MetadataOnly,
        }],
        invalidation_key: semantic_invalidation_key(),
        provenance: semantic_provenance(SemanticRecordSource::TreeSitter),
        freshness: SemanticFreshnessState::Fresh,
        schema_version: 1,
    };

    let value = serde_json::to_value((&token, &file_identity, &symbol_map, &graph_record))
        .expect("serialize semantic records");
    assert_eq!(value[0]["token_id"], "34343434-3434-3434-3434-343434343434");
    assert_eq!(value[0]["reason"], "SnapshotSuperseded");
    assert_eq!(value[1]["content_hash"]["value"], "content");
    assert_eq!(value[1]["disk_fingerprint"]["value"], "disk");
    assert_eq!(value[2]["symbol_id"], "symbol:main");
    assert_eq!(
        value[2]["invalidation_key"]["grammar_version"],
        "tree-sitter-rust@1"
    );
    assert_eq!(value[2]["invalidation_key"]["model_version"], "ranker@0");
    assert_eq!(value[2]["invalidation_key"]["privacy_scope"], "Workspace");
    assert_eq!(value[3]["kind"], "CallEdge");
    assert_eq!(value[3]["properties"][0]["redaction"], "MetadataOnly");
    assert_eq!(value[3]["provenance"]["source"], "TreeSitter");

    let roundtrip: (
        SemanticCancellationToken,
        SemanticFileFingerprintIdentity,
        SymbolFileMapRecord,
        SemanticGraphRecord,
    ) = serde_json::from_value(value).expect("deserialize semantic records");
    assert!(matches!(
        roundtrip.0.reason,
        Some(SemanticCancellationReason::SnapshotSuperseded)
    ));
    assert_eq!(roundtrip.1.file_id, FileId(33));
    assert_eq!(
        roundtrip.2.symbol_id,
        SemanticSymbolId("symbol:main".to_string())
    );
    assert!(matches!(
        roundtrip.3.kind,
        SemanticGraphRecordKind::CallEdge
    ));

    let mut missing = serde_json::to_value(semantic_invalidation_key()).expect("serialize key");
    remove_required_field::<SemanticInvalidationKey>(&mut missing, "content_hash");
}

#[test]
fn dto_contracts_workspace_discovery_dtos_golden_and_required_fields() {
    let policy = WorkspaceDiscoveryPolicyDecision {
        decision: WorkspaceDiscoveryDecision::MetadataOnly,
        skip_reason: Some(WorkspaceDiscoverySkipReason::Generated),
        path_policy: WorkspaceDiscoveryPathPolicyResult::WorkspaceAllowed,
        trust: WorkspaceDiscoveryTrustResult::Trusted,
        generated: true,
        binary: false,
        vendored: false,
        oversized: false,
        metadata_only: true,
    };
    let record = WorkspaceDiscoveryRecord {
        schema_version: 1,
        workspace_id: Some(WorkspaceId(11)),
        workspace_root_id: Some(WorkspaceRootId(22)),
        workspace_generation: WorkspaceGeneration(77),
        identity: Some(file_identity()),
        path: Some(CanonicalPath("C:/repo/src/generated.rs".to_string())),
        display_path: Some("src/generated.rs".to_string()),
        metadata: Some(FileMetadata {
            canonical_path: CanonicalPath("C:/repo/src/generated.rs".to_string()),
            file_id: Some(FileId(33)),
            workspace_id: Some(WorkspaceId(11)),
            kind: FileKind::File,
            size_bytes: Some(1234),
            modified_at: Some(TimestampMillis(9876)),
            read_only: false,
            permissions: Some("metadata-only".to_string()),
            hash: Some("sha256:file".to_string()),
            fingerprint: Some(fingerprint("disk")),
            content_version: Some(FileContentVersion(44)),
            workspace_generation: Some(WorkspaceGeneration(77)),
            schema_version: 1,
        }),
        policy,
        language_hint: Some(LanguageId("rust".to_string())),
        privacy_scope: SemanticPrivacyScope::MetadataOnly,
        content_fingerprint: Some(fingerprint("disk")),
        content_hash: None,
        change_kind: Some(WorkspaceDiscoveryChangeKind::PolicyChanged),
    };
    let snapshot = WorkspaceDiscoverySnapshot {
        schema_version: 1,
        workspace_id: WorkspaceId(11),
        workspace_root_id: Some(WorkspaceRootId(22)),
        workspace_generation: WorkspaceGeneration(77),
        captured_at: TimestampMillis(12345),
        records: vec![record.clone()],
        diagnostics: vec![diagnostic("workspace-discovery")],
    };
    let delta = WorkspaceDiscoveryDelta {
        schema_version: 1,
        workspace_id: WorkspaceId(11),
        workspace_generation: WorkspaceGeneration(78),
        sequence: EventSequence(9),
        records: vec![WorkspaceDiscoveryRecord {
            change_kind: Some(WorkspaceDiscoveryChangeKind::Deleted),
            policy: WorkspaceDiscoveryPolicyDecision {
                decision: WorkspaceDiscoveryDecision::Excluded,
                skip_reason: Some(WorkspaceDiscoverySkipReason::Deleted),
                path_policy: WorkspaceDiscoveryPathPolicyResult::WorkspaceAllowed,
                trust: WorkspaceDiscoveryTrustResult::Trusted,
                generated: false,
                binary: false,
                vendored: false,
                oversized: false,
                metadata_only: true,
            },
            ..record.clone()
        }],
        diagnostics: Vec::new(),
    };

    let value =
        serde_json::to_value((&snapshot, &delta)).expect("serialize workspace discovery dtos");
    let expected = json!([
        {
            "schema_version": 1,
            "workspace_id": 11,
            "workspace_root_id": 22,
            "workspace_generation": 77,
            "captured_at": 12345,
            "records": [{
                "schema_version": 1,
                "workspace_id": 11,
                "workspace_root_id": 22,
                "workspace_generation": 77,
                "identity": {
                    "file_id": 33,
                    "workspace_id": 11,
                    "canonical_path": "C:/repo/src/main.rs",
                    "content_version": 44,
                    "content_hash": "sha256:file"
                },
                "path": "C:/repo/src/generated.rs",
                "display_path": "src/generated.rs",
                "metadata": {
                    "canonical_path": "C:/repo/src/generated.rs",
                    "file_id": 33,
                    "workspace_id": 11,
                    "kind": "File",
                    "size_bytes": 1234,
                    "modified_at": 9876,
                    "read_only": false,
                    "permissions": "metadata-only",
                    "hash": "sha256:file",
                    "fingerprint": {"algorithm": "sha256", "value": "disk"},
                    "content_version": 44,
                    "workspace_generation": 77,
                    "schema_version": 1
                },
                "policy": {
                    "decision": "MetadataOnly",
                    "skip_reason": "Generated",
                    "path_policy": "WorkspaceAllowed",
                    "trust": "Trusted",
                    "generated": true,
                    "binary": false,
                    "vendored": false,
                    "oversized": false,
                    "metadata_only": true
                },
                "language_hint": "rust",
                "privacy_scope": "MetadataOnly",
                "content_fingerprint": {"algorithm": "sha256", "value": "disk"},
                "content_hash": null,
                "change_kind": "PolicyChanged"
            }],
            "diagnostics": [{
                "code": "workspace-discovery",
                "message": "diagnostic workspace-discovery",
                "severity": "Warning",
                "path": "C:/repo/src/main.rs",
                "range": {
                    "start": {"line": 1, "character": 2, "byte_offset": 10, "utf16_offset": 8},
                    "end": {"line": 1, "character": 6, "byte_offset": 14, "utf16_offset": 12}
                }
            }]
        },
        {
            "schema_version": 1,
            "workspace_id": 11,
            "workspace_generation": 78,
            "sequence": 9,
            "records": [{
                "schema_version": 1,
                "workspace_id": 11,
                "workspace_root_id": 22,
                "workspace_generation": 77,
                "identity": {
                    "file_id": 33,
                    "workspace_id": 11,
                    "canonical_path": "C:/repo/src/main.rs",
                    "content_version": 44,
                    "content_hash": "sha256:file"
                },
                "path": "C:/repo/src/generated.rs",
                "display_path": "src/generated.rs",
                "metadata": {
                    "canonical_path": "C:/repo/src/generated.rs",
                    "file_id": 33,
                    "workspace_id": 11,
                    "kind": "File",
                    "size_bytes": 1234,
                    "modified_at": 9876,
                    "read_only": false,
                    "permissions": "metadata-only",
                    "hash": "sha256:file",
                    "fingerprint": {"algorithm": "sha256", "value": "disk"},
                    "content_version": 44,
                    "workspace_generation": 77,
                    "schema_version": 1
                },
                "policy": {
                    "decision": "Excluded",
                    "skip_reason": "Deleted",
                    "path_policy": "WorkspaceAllowed",
                    "trust": "Trusted",
                    "generated": false,
                    "binary": false,
                    "vendored": false,
                    "oversized": false,
                    "metadata_only": true
                },
                "language_hint": "rust",
                "privacy_scope": "MetadataOnly",
                "content_fingerprint": {"algorithm": "sha256", "value": "disk"},
                "content_hash": null,
                "change_kind": "Deleted"
            }],
            "diagnostics": []
        }
    ]);
    assert_eq!(value, expected);

    let roundtrip: (WorkspaceDiscoverySnapshot, WorkspaceDiscoveryDelta) =
        serde_json::from_value(value.clone()).expect("deserialize workspace discovery dtos");
    assert_eq!(
        roundtrip.0.records[0].policy.skip_reason,
        Some(WorkspaceDiscoverySkipReason::Generated)
    );
    assert_eq!(
        roundtrip.1.records[0].change_kind,
        Some(WorkspaceDiscoveryChangeKind::Deleted)
    );

    let mut missing = value[0].clone();
    remove_required_field::<WorkspaceDiscoverySnapshot>(&mut missing, "records");
}

#[test]
fn dto_contracts_semantic_query_request_response_golden() {
    let request = SemanticQueryRequest {
        query_id: semantic_query_id(),
        kind: SemanticQueryKind::RefactoringPreview,
        scope: SemanticQueryScope {
            workspace_id: WorkspaceId(11),
            file_ids: vec![FileId(33)],
            paths: vec![CanonicalPath("C:/repo/src/main.rs".to_string())],
            language_ids: vec![LanguageId("rust".to_string())],
            privacy_scope: SemanticPrivacyScope::Workspace,
        },
        position: Some(TextCoordinate {
            line: 1,
            character: 4,
            byte_offset: Some(12),
            utf16_offset: Some(10),
        }),
        text_query_hash: Some(fingerprint("query")),
        limit: 25,
        cancellation_token: cancellation_token_id(),
        freshness_policy: SemanticQueryFreshnessPolicy::AllowStale,
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        schema_version: 1,
    };
    let response = SemanticQueryResponse {
        query_id: semantic_query_id(),
        workspace_id: WorkspaceId(11),
        status: SemanticQueryStatus::Degraded,
        results: vec![SemanticQueryResult {
            result_id: SemanticRecordId("result:refactor:rename".to_string()),
            kind: SemanticQueryResultKind::ProposalPreview,
            label: "Rename main to renamed".to_string(),
            file_id: Some(FileId(33)),
            path: Some(CanonicalPath("C:/repo/src/main.rs".to_string())),
            range: Some(protocol_range()),
            score_basis_points: 8750,
            freshness: semantic_freshness(SemanticFreshnessState::Stale),
            provenance: semantic_provenance(SemanticRecordSource::Lsp),
            related_record_ids: vec![SemanticRecordId("record:symbol:main".to_string())],
            proposal_preview: Some(payload_summary()),
        }],
        diagnostics: vec![diagnostic("semantic-query")],
        next_page_token: Some("page-2".to_string()),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        schema_version: 1,
    };

    let value = serde_json::to_value((&request, &response)).expect("serialize semantic query dtos");
    let expected = json!([
        {
            "query_id": "56565656-5656-5656-5656-565656565656",
            "kind": "RefactoringPreview",
            "scope": {
                "workspace_id": 11,
                "file_ids": [33],
                "paths": ["C:/repo/src/main.rs"],
                "language_ids": ["rust"],
                "privacy_scope": "Workspace"
            },
            "position": {"line": 1, "character": 4, "byte_offset": 12, "utf16_offset": 10},
            "text_query_hash": {"algorithm": "sha256", "value": "query"},
            "limit": 25,
            "cancellation_token": "34343434-3434-3434-3434-343434343434",
            "freshness_policy": "AllowStale",
            "correlation_id": 901,
            "causality_id": "cccccccc-cccc-cccc-cccc-cccccccccccc",
            "schema_version": 1
        },
        {
            "query_id": "56565656-5656-5656-5656-565656565656",
            "workspace_id": 11,
            "status": "Degraded",
            "results": [{
                "result_id": "result:refactor:rename",
                "kind": "ProposalPreview",
                "label": "Rename main to renamed",
                "file_id": 33,
                "path": "C:/repo/src/main.rs",
                "range": {
                    "start": {"line": 1, "character": 2, "byte_offset": 10, "utf16_offset": 8},
                    "end": {"line": 1, "character": 6, "byte_offset": 14, "utf16_offset": 12}
                },
                "score_basis_points": 8750,
                "freshness": {
                    "state": "Stale",
                    "key": {
                        "workspace_id": 11,
                        "file_id": 33,
                        "snapshot_id": 66,
                        "file_content_version": 44,
                        "workspace_generation": 77,
                        "content_hash": {"algorithm": "sha256", "value": "content"},
                        "grammar_version": "tree-sitter-rust@1",
                        "model_version": "ranker@0",
                        "privacy_scope": "Workspace",
                        "schema_version": 1
                    },
                    "degraded_reasons": ["lsp_unavailable"],
                    "observed_at": 2222
                },
                "provenance": {
                    "source": "Lsp",
                    "server_id": 7,
                    "extraction_version": "phase3-contract-v1",
                    "confidence_basis_points": 9000
                },
                "related_record_ids": ["record:symbol:main"],
                "proposal_preview": {
                    "kind": "SaveFile",
                    "affected_files": [33],
                    "title": "save main.rs",
                    "byte_count": 1234
                }
            }],
            "diagnostics": [{
                "code": "semantic-query",
                "message": "diagnostic semantic-query",
                "severity": "Warning",
                "path": "C:/repo/src/main.rs",
                "range": {
                    "start": {"line": 1, "character": 2, "byte_offset": 10, "utf16_offset": 8},
                    "end": {"line": 1, "character": 6, "byte_offset": 14, "utf16_offset": 12}
                }
            }],
            "next_page_token": "page-2",
            "correlation_id": 901,
            "causality_id": "cccccccc-cccc-cccc-cccc-cccccccccccc",
            "schema_version": 1
        }
    ]);
    assert_eq!(value, expected);

    let roundtrip: (SemanticQueryRequest, SemanticQueryResponse) =
        serde_json::from_value(value.clone()).expect("deserialize semantic query dtos");
    assert!(matches!(
        roundtrip.0.kind,
        SemanticQueryKind::RefactoringPreview
    ));
    assert!(matches!(roundtrip.1.status, SemanticQueryStatus::Degraded));
    assert!(matches!(
        roundtrip.1.results[0].kind,
        SemanticQueryResultKind::ProposalPreview
    ));
    assert!(matches!(
        roundtrip.1.results[0].freshness.state,
        SemanticFreshnessState::Stale
    ));

    let mut missing = value[0].clone();
    remove_required_field::<SemanticQueryRequest>(&mut missing, "cancellation_token");
}

#[test]
fn dto_contracts_semantic_fabric_scheduling_metadata_only_roundtrip() {
    let descriptor = SemanticFabricDescriptorReference {
        source_kind: SemanticMetadataSourceKind::LeaseChunks,
        snapshot_id: Some(SnapshotId(66)),
        content_hash: fingerprint("content"),
        byte_len: Some(4096),
        ranges: vec![ByteRange::new(0, 128)],
        chunks: vec![SemanticMetadataChunkReference {
            snapshot_id: SnapshotId(66),
            chunk_index: 0,
            byte_range: ByteRange::new(0, 128),
            line_range: LineIndexRange { start: 0, end: 8 },
            byte_len: 128,
            chunk_hash: chunk_hash("chunk-0"),
            lease_present: true,
            schema_version: 1,
        }],
        schema_version: 1,
    };
    let freshness_key = SemanticMetadataFreshnessKey {
        workspace_id: WorkspaceId(11),
        file_id: FileId(33),
        language_id: LanguageId("rust".to_string()),
        snapshot_id: Some(SnapshotId(66)),
        file_content_version: FileContentVersion(44),
        workspace_generation: WorkspaceGeneration(77),
        content_hash: fingerprint("content"),
        grammar_version: Some(SemanticGrammarVersion(
            "lexical-fallback-grammar-v1".to_string(),
        )),
        model_version: Some(SemanticModelVersion(
            "semantic-ranking-metadata-v1".to_string(),
        )),
        parser_version: "devil-index-lexical-v1".to_string(),
        privacy_scope: SemanticPrivacyScope::Workspace,
        descriptor: SemanticMetadataDescriptorIdentity {
            source_kind: descriptor.source_kind,
            snapshot_id: descriptor.snapshot_id,
            content_hash: descriptor.content_hash.clone(),
            byte_len: descriptor.byte_len,
            ranges: descriptor.ranges.clone(),
            chunks: descriptor.chunks.clone(),
            schema_version: descriptor.schema_version,
        },
        schema_version: 1,
    };
    let request = SemanticFabricJobRequest {
        job_id: "semantic-fabric:33".to_string(),
        source_kind: SemanticFabricWorkSourceKind::SnapshotLeaseMetadata,
        trigger: SemanticFabricSchedulingTrigger::ForegroundViewport,
        workspace_id: WorkspaceId(11),
        file_id: FileId(33),
        language_id: LanguageId("rust".to_string()),
        file_identity: SemanticFileFingerprintIdentity {
            workspace_id: WorkspaceId(11),
            file_id: FileId(33),
            canonical_path: CanonicalPath("C:/repo/src/main.rs".to_string()),
            file_content_version: FileContentVersion(44),
            workspace_generation: WorkspaceGeneration(77),
            content_hash: fingerprint("content"),
            disk_fingerprint: Some(fingerprint("disk")),
            byte_len: Some(4096),
            modified_at: Some(TimestampMillis(9999)),
            privacy_scope: SemanticPrivacyScope::Workspace,
            schema_version: 1,
        },
        expected_freshness_key: freshness_key.clone(),
        persisted_freshness_key: Some(freshness_key),
        descriptor,
        privacy: SemanticFabricPrivacyLabel {
            privacy_scope: SemanticPrivacyScope::Workspace,
            metadata_only: true,
            redaction: RedactionHint::MetadataOnly,
            schema_version: 1,
        },
        dependency_hints: vec![SemanticFabricDependencyHint {
            file_id: FileId(34),
            label_hash: fingerprint("dep"),
            confidence_basis_points: 8750,
            schema_version: 1,
        }],
        cancellation: SemanticCancellationToken {
            token_id: CancellationTokenId(
                Uuid::parse_str("abababab-abab-abab-abab-abababababab").unwrap(),
            ),
            workspace_id: WorkspaceId(11),
            file_id: Some(FileId(33)),
            snapshot_id: Some(SnapshotId(66)),
            content_hash: Some(fingerprint("content")),
            workspace_generation: Some(WorkspaceGeneration(77)),
            privacy_scope: SemanticPrivacyScope::Workspace,
            reason: None,
            issued_at: TimestampMillis(1111),
            expires_at: Some(TimestampMillis(2222)),
            schema_version: 1,
        },
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        schema_version: 1,
    };
    let plan = SemanticFabricSchedulePlan {
        decisions: vec![SemanticFabricSchedulingDecision {
            job_id: "semantic-fabric:33".to_string(),
            action: SemanticFabricSchedulingAction::Coalesce,
            priority: SemanticFabricPriority::ForegroundViewport,
            priority_score: 925,
            freshness_state: SemanticFreshnessState::Fresh,
            invalidation_causes: Vec::new(),
            cancellation_reason: None,
            metadata_only: true,
            queue_depth: 0,
            diagnostics: vec![diagnostic("semantic-fabric")],
            schema_version: 1,
        }],
        admitted_count: 0,
        capacity: 8,
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        schema_version: 1,
    };

    let value = serde_json::to_value((&request, &plan)).expect("serialize semantic fabric DTOs");
    assert_eq!(value[0]["source_kind"], json!("SnapshotLeaseMetadata"));
    assert_eq!(
        value[0]["descriptor"]["chunks"][0]["lease_present"],
        json!(true)
    );
    assert_eq!(value[1]["decisions"][0]["action"], json!("Coalesce"));
    assert!(!format!("{value:?}").contains("fn "));
    assert!(!format!("{value:?}").contains("source_body"));

    let roundtrip: (SemanticFabricJobRequest, SemanticFabricSchedulePlan) =
        serde_json::from_value(value.clone()).expect("deserialize semantic fabric DTOs");
    assert_eq!(
        roundtrip.0.source_kind,
        SemanticFabricWorkSourceKind::SnapshotLeaseMetadata
    );
    assert_eq!(
        roundtrip.1.decisions[0].action,
        SemanticFabricSchedulingAction::Coalesce
    );

    let mut missing = value[0].clone();
    remove_required_field::<SemanticFabricJobRequest>(&mut missing, "expected_freshness_key");
}

#[test]
fn dto_contracts_semantic_and_workspace_service_envelopes_roundtrip() {
    let token = SemanticCancellationToken {
        token_id: cancellation_token_id(),
        workspace_id: WorkspaceId(11),
        file_id: Some(FileId(33)),
        snapshot_id: Some(SnapshotId(66)),
        content_hash: Some(fingerprint("content")),
        workspace_generation: Some(WorkspaceGeneration(77)),
        privacy_scope: SemanticPrivacyScope::Workspace,
        reason: Some(SemanticCancellationReason::UserCancelled),
        issued_at: TimestampMillis(1111),
        expires_at: Some(TimestampMillis(2222)),
        schema_version: 1,
    };
    let semantic_request = SemanticRequest::Cancel(token.clone());
    let semantic_response = SemanticResponse::Cancelled(token);
    let workspace_request = WorkspaceRequest::BuildSemanticDiscoveryDelta {
        workspace_id: WorkspaceId(11),
        events: vec![WatcherEvent {
            workspace_id: WorkspaceId(11),
            kind: WatcherEventKind::Modified,
            path: CanonicalPath("C:/repo/src/main.rs".to_string()),
            old_path: None,
            sequence: EventSequence(9),
        }],
    };

    let value = serde_json::to_value((&semantic_request, &semantic_response, &workspace_request))
        .expect("serialize service envelopes");
    assert_eq!(value[0]["Cancel"]["privacy_scope"], json!("Workspace"));
    assert_eq!(value[1]["Cancelled"]["reason"], json!("UserCancelled"));
    assert_eq!(
        value[2]["BuildSemanticDiscoveryDelta"]["events"][0]["kind"],
        json!("Modified")
    );

    let roundtrip: (SemanticRequest, SemanticResponse, WorkspaceRequest) =
        serde_json::from_value(value).expect("deserialize service envelopes");
    assert!(matches!(roundtrip.0, SemanticRequest::Cancel(_)));
    assert!(matches!(roundtrip.1, SemanticResponse::Cancelled(_)));
    assert!(matches!(
        roundtrip.2,
        WorkspaceRequest::BuildSemanticDiscoveryDelta { .. }
    ));
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
fn dto_contracts_batch_proposal_payload_golden_and_required_fields() {
    let payload = batch_payload();
    let envelope = ProposalPayload::Batch(payload);
    let value = serde_json::to_value(&envelope).expect("serialize batch proposal");
    let expected = json!({
        "Batch": {
            "batch_id": "55555555-5555-5555-5555-555555555555",
            "atomicity": "PrepareAllBeforeMutate",
            "rollback_policy": "Required",
            "target_coverage": {
                "coverage_kind": "Complete",
                "targets": [
                    {
                        "target_id": "target-buffer-main",
                        "kind": "OpenBuffer",
                        "workspace_id": 11,
                        "file_id": 33,
                        "buffer_id": 22,
                        "path": "C:/repo/src/main.rs",
                        "terminal_session_id": null,
                        "plugin_id": null,
                        "remote_authority": null,
                        "collaboration_session_id": null,
                        "byte_ranges": [{"start": 10, "end": 14}],
                        "redaction_hints": ["MetadataOnly"]
                    },
                    {
                        "target_id": "target-file-lib",
                        "kind": "PathOnly",
                        "workspace_id": 11,
                        "file_id": null,
                        "buffer_id": null,
                        "path": "C:/repo/src/lib.rs",
                        "terminal_session_id": null,
                        "plugin_id": null,
                        "remote_authority": null,
                        "collaboration_session_id": null,
                        "byte_ranges": [],
                        "redaction_hints": ["MetadataOnly"]
                    }
                ],
                "omitted_target_count": 0,
                "redaction_hints": ["MetadataOnly"]
            },
            "items": [
                {
                    "order": 0,
                    "item_id": "item-edit-main",
                    "payload": {
                        "TextEdit": {
                            "file_id": 33,
                            "edits": {
                                "edits": [{
                                    "range": {
                                        "start": {"value": 10, "encoding": "Byte"},
                                        "end": {"value": 14, "encoding": "Byte"}
                                    },
                                    "replacement": "main"
                                }]
                            }
                        }
                    },
                    "target_ids": ["target-buffer-main"],
                    "required_capability": "editor.write",
                    "rollback_step_ids": ["rollback-edit-main"]
                },
                {
                    "order": 1,
                    "item_id": "item-create-lib",
                    "payload": {
                        "CreateFile": {
                            "path": "C:/repo/src/lib.rs",
                            "initial_content": null
                        }
                    },
                    "target_ids": ["target-file-lib"],
                    "required_capability": "fs.write",
                    "rollback_step_ids": ["rollback-create-lib"]
                }
            ],
            "dependency_edges": [{
                "prerequisite_item_id": "item-edit-main",
                "dependent_item_id": "item-create-lib",
                "kind": "RequiresValidation"
            }],
            "rollback_steps": [
                {
                    "order": 0,
                    "step_id": "rollback-create-lib",
                    "item_id": "item-create-lib",
                    "target_id": "target-file-lib",
                    "action": "DeleteCreatedFile",
                    "expected_preconditions": {
                        "file_version": null,
                        "buffer_version": null,
                        "snapshot_id": null,
                        "generation": null,
                        "file_content_version": null,
                        "workspace_generation": null,
                        "expected_fingerprint": null,
                        "expected_file_length": null,
                        "expected_modified_at": null
                    },
                    "diagnostics": []
                },
                {
                    "order": 1,
                    "step_id": "rollback-edit-main",
                    "item_id": "item-edit-main",
                    "target_id": "target-buffer-main",
                    "action": "EditorUndoGroup",
                    "expected_preconditions": {
                        "file_version": 44,
                        "buffer_version": 55,
                        "snapshot_id": 66,
                        "generation": 77,
                        "file_content_version": 44,
                        "workspace_generation": 77,
                        "expected_fingerprint": {"algorithm": "sha256", "value": "expected"},
                        "expected_file_length": 1234,
                        "expected_modified_at": 9876
                    },
                    "diagnostics": [{
                        "code": "rollback",
                        "message": "diagnostic rollback",
                        "severity": "Warning",
                        "path": "C:/repo/src/main.rs",
                        "range": {
                            "start": {"line": 1, "character": 2, "byte_offset": 10, "utf16_offset": 8},
                            "end": {"line": 1, "character": 6, "byte_offset": 14, "utf16_offset": 12}
                        }
                    }]
                }
            ],
            "partial_failures": [{
                "item_id": "item-create-lib",
                "target_id": "target-file-lib",
                "reason": "ApplyFailed",
                "disposition": "RolledBack",
                "diagnostics": [{
                    "code": "partial",
                    "message": "diagnostic partial",
                    "severity": "Warning",
                    "path": "C:/repo/src/main.rs",
                    "range": {
                        "start": {"line": 1, "character": 2, "byte_offset": 10, "utf16_offset": 8},
                        "end": {"line": 1, "character": 6, "byte_offset": 14, "utf16_offset": 12}
                    }
                }]
            }],
            "preview_warnings": [{
                "code": "proposal.preview.rollback_required",
                "kind": "RollbackBestEffort",
                "message": "batch includes rollback metadata",
                "target_id": "target-file-lib",
                "redaction_hints": ["MetadataOnly"]
            }],
            "schema_version": 1
        }
    });
    assert_eq!(value, expected);

    let roundtrip: ProposalPayload =
        serde_json::from_value(value.clone()).expect("deserialize batch proposal payload");
    match roundtrip {
        ProposalPayload::Batch(batch) => {
            assert_eq!(batch.items[0].order, 0);
            assert_eq!(batch.items[1].order, 1);
            assert!(matches!(
                batch.atomicity,
                ProposalBatchAtomicity::PrepareAllBeforeMutate
            ));
            assert_eq!(batch.target_coverage.targets.len(), 2);
            assert_eq!(batch.rollback_steps.len(), 2);
            assert_eq!(batch.partial_failures.len(), 1);
        }
        _ => panic!("unexpected payload"),
    }

    let mut missing = value["Batch"].clone();
    remove_required_field::<BatchProposalPayload>(&mut missing, "schema_version");
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
        ProposalResponse::Cancelled {
            transition: transition(ProposalLifecycleState::Cancelled),
            reason: ProposalCancellationReason::UserCancelled,
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
        "Cancelled",
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
    assert_eq!(value[11]["Cancelled"]["reason"], "UserCancelled");

    let roundtrip: Vec<ProposalResponse> =
        serde_json::from_value(value).expect("deserialize lifecycle responses");
    assert!(matches!(roundtrip[0], ProposalResponse::Created(_)));
    assert!(matches!(roundtrip[10], ProposalResponse::Conflict { .. }));
    assert!(matches!(roundtrip[11], ProposalResponse::Cancelled { .. }));
}

#[test]
fn dto_contracts_proposal_lifecycle_commands_golden_and_required_fields() {
    let commands = vec![
        ProposalRequest::Approve(ProposalLifecycleCommand {
            proposal_id: devil_protocol::ProposalId(700),
            action: ProposalLifecycleAction::Approve,
            principal: PrincipalId("principal-1".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(901),
            causality_id: causality_id(),
            reason: None,
            diagnostics: vec![],
            requested_at: TimestampMillis(1700),
            schema_version: 1,
        }),
        ProposalRequest::Reject(ProposalLifecycleCommand {
            proposal_id: devil_protocol::ProposalId(700),
            action: ProposalLifecycleAction::Reject,
            principal: PrincipalId("principal-1".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(901),
            causality_id: causality_id(),
            reason: Some(ProposalLifecycleCommandReason::Rejection(
                ProposalRejectionReason::UserRejected,
            )),
            diagnostics: vec![diagnostic("reject")],
            requested_at: TimestampMillis(1701),
            schema_version: 1,
        }),
        ProposalRequest::Cancel(ProposalLifecycleCommand {
            proposal_id: devil_protocol::ProposalId(700),
            action: ProposalLifecycleAction::Cancel,
            principal: PrincipalId("principal-1".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(901),
            causality_id: causality_id(),
            reason: Some(ProposalLifecycleCommandReason::Cancellation(
                ProposalCancellationReason::Superseded,
            )),
            diagnostics: vec![],
            requested_at: TimestampMillis(1702),
            schema_version: 1,
        }),
        ProposalRequest::Rollback(ProposalLifecycleCommand {
            proposal_id: devil_protocol::ProposalId(700),
            action: ProposalLifecycleAction::Rollback,
            principal: PrincipalId("principal-1".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            correlation_id: CorrelationId(901),
            causality_id: causality_id(),
            reason: Some(ProposalLifecycleCommandReason::Rollback(
                ProposalRollbackReason::UserRequested,
            )),
            diagnostics: vec![diagnostic("rollback")],
            requested_at: TimestampMillis(1703),
            schema_version: 1,
        }),
    ];

    let value = serde_json::to_value(&commands).expect("serialize lifecycle commands");
    let expected = json!([
        {
            "Approve": {
                "proposal_id": 700,
                "action": "Approve",
                "principal": "principal-1",
                "capability": "fs.write",
                "correlation_id": 901,
                "causality_id": "cccccccc-cccc-cccc-cccc-cccccccccccc",
                "reason": null,
                "diagnostics": [],
                "requested_at": 1700,
                "schema_version": 1
            }
        },
        {
            "Reject": {
                "proposal_id": 700,
                "action": "Reject",
                "principal": "principal-1",
                "capability": "fs.write",
                "correlation_id": 901,
                "causality_id": "cccccccc-cccc-cccc-cccc-cccccccccccc",
                "reason": {"Rejection": "UserRejected"},
                "diagnostics": [{
                    "code": "reject",
                    "message": "diagnostic reject",
                    "severity": "Warning",
                    "path": "C:/repo/src/main.rs",
                    "range": {
                        "start": {"line": 1, "character": 2, "byte_offset": 10, "utf16_offset": 8},
                        "end": {"line": 1, "character": 6, "byte_offset": 14, "utf16_offset": 12}
                    }
                }],
                "requested_at": 1701,
                "schema_version": 1
            }
        },
        {
            "Cancel": {
                "proposal_id": 700,
                "action": "Cancel",
                "principal": "principal-1",
                "capability": "fs.write",
                "correlation_id": 901,
                "causality_id": "cccccccc-cccc-cccc-cccc-cccccccccccc",
                "reason": {"Cancellation": "Superseded"},
                "diagnostics": [],
                "requested_at": 1702,
                "schema_version": 1
            }
        },
        {
            "Rollback": {
                "proposal_id": 700,
                "action": "Rollback",
                "principal": "principal-1",
                "capability": "fs.write",
                "correlation_id": 901,
                "causality_id": "cccccccc-cccc-cccc-cccc-cccccccccccc",
                "reason": {"Rollback": "UserRequested"},
                "diagnostics": [{
                    "code": "rollback",
                    "message": "diagnostic rollback",
                    "severity": "Warning",
                    "path": "C:/repo/src/main.rs",
                    "range": {
                        "start": {"line": 1, "character": 2, "byte_offset": 10, "utf16_offset": 8},
                        "end": {"line": 1, "character": 6, "byte_offset": 14, "utf16_offset": 12}
                    }
                }],
                "requested_at": 1703,
                "schema_version": 1
            }
        }
    ]);
    assert_eq!(value, expected);

    let roundtrip: Vec<ProposalRequest> =
        serde_json::from_value(value.clone()).expect("deserialize lifecycle commands");
    assert!(matches!(roundtrip[0], ProposalRequest::Approve(_)));
    assert!(matches!(roundtrip[1], ProposalRequest::Reject(_)));
    assert!(matches!(roundtrip[2], ProposalRequest::Cancel(_)));
    assert!(matches!(roundtrip[3], ProposalRequest::Rollback(_)));

    let mut missing = value[0]["Approve"].clone();
    remove_required_field::<ProposalLifecycleCommand>(&mut missing, "schema_version");
}

#[test]
fn dto_contracts_proposal_ledger_projection_golden_metadata_only() {
    let dto = ProposalLedgerProjection {
        rows: vec![ProposalLedgerRow {
            proposal_id: devil_protocol::ProposalId(700),
            workspace_id: Some(WorkspaceId(11)),
            title: "rename symbol".to_string(),
            payload_kind: ProposalPayloadKind::WorkspaceEdit,
            lifecycle: ProposalLifecycleStateDisplay {
                state: ProposalLifecycleState::Previewed,
                label: "Previewed".to_string(),
                description: "ready for review".to_string(),
            },
            principal: PrincipalId("principal-1".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            created_at: TimestampMillis(1000),
            updated_at: TimestampMillis(1700),
            expires_at: Some(TimestampMillis(2000)),
            risk_label: ProposalRiskLabel::Medium,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            rollback: ProposalRollbackAvailability::BestEffort,
            target_coverage: batch_target_coverage(),
            context_manifest: ProposalContextManifestSummary {
                manifest_id: "manifest:rename-symbol".to_string(),
                category_count: 2,
                total_item_count: 3,
                omitted_item_count: 1,
                categories: vec![
                    ProposalContextManifestEntrySummary {
                        category: "files".to_string(),
                        item_count: 2,
                        omitted_item_count: 0,
                        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                        manifest_hash: Some(fingerprint("files-manifest")),
                        redaction_hints: vec![RedactionHint::MetadataOnly],
                    },
                    ProposalContextManifestEntrySummary {
                        category: "diagnostics".to_string(),
                        item_count: 1,
                        omitted_item_count: 1,
                        privacy_label: ProposalPrivacyLabel::RedactedSensitive,
                        manifest_hash: Some(fingerprint("diagnostics-manifest")),
                        redaction_hints: vec![RedactionHint::Full],
                    },
                ],
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            diff_summary: ProposalDiffSummary {
                kind: ProposalDiffSummaryKind::WorkspaceEdit,
                target_count: 2,
                hunk_count: 3,
                inserted_line_count: 10,
                deleted_line_count: 4,
                omitted_hunk_count: 99,
                full_source_redacted: true,
                diff_hash: Some(fingerprint("diff-summary")),
                chunks: vec![ProposalDiffChunkDescriptor {
                    chunk_id: "chunk-0".to_string(),
                    target_id: Some("target-buffer-main".to_string()),
                    byte_range: Some(devil_protocol::ByteRange::new(10, 14)),
                    changed_line_count: 2,
                    inserted_line_count: 1,
                    deleted_line_count: 1,
                    content_hash: Some(chunk_hash("ledger-chunk")),
                }],
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            preview_warnings: vec![ProposalPreviewWarning {
                code: "proposal.preview.raw_source_redacted".to_string(),
                kind: ProposalPreviewWarningKind::RawSourceRedacted,
                message: "raw source is not present in the ledger projection".to_string(),
                target_id: Some("target-buffer-main".to_string()),
                redaction_hints: vec![RedactionHint::MetadataOnly],
            }],
            diagnostics: vec![diagnostic("ledger")],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        selected_proposal_id: Some(devil_protocol::ProposalId(700)),
        omitted_row_count: 5,
        generated_at: TimestampMillis(1800),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let value = serde_json::to_value(&dto).expect("serialize ledger projection");
    let expected = json!({
        "rows": [{
            "proposal_id": 700,
            "workspace_id": 11,
            "title": "rename symbol",
            "payload_kind": "WorkspaceEdit",
            "lifecycle": {
                "state": "Previewed",
                "label": "Previewed",
                "description": "ready for review"
            },
            "principal": "principal-1",
            "capability": "fs.write",
            "created_at": 1000,
            "updated_at": 1700,
            "expires_at": 2000,
            "risk_label": "Medium",
            "privacy_label": "WorkspaceMetadata",
            "rollback": "BestEffort",
            "target_coverage": value["rows"][0]["target_coverage"].clone(),
            "context_manifest": value["rows"][0]["context_manifest"].clone(),
            "diff_summary": value["rows"][0]["diff_summary"].clone(),
            "preview_warnings": value["rows"][0]["preview_warnings"].clone(),
            "diagnostics": value["rows"][0]["diagnostics"].clone(),
            "redaction_hints": ["MetadataOnly"],
            "schema_version": 1
        }],
        "selected_proposal_id": 700,
        "omitted_row_count": 5,
        "generated_at": 1800,
        "redaction_hints": ["MetadataOnly"],
        "schema_version": 1
    });
    assert_eq!(value, expected);
    assert_eq!(
        value["rows"][0]["diff_summary"]["full_source_redacted"],
        true
    );
    let serialized = value.to_string();
    assert!(serialized.contains("diff-summary"));
    assert!(!serialized.contains("fn main"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("prompt"));

    let roundtrip: ProposalLedgerProjection =
        serde_json::from_value(value.clone()).expect("deserialize ledger projection");
    assert_eq!(roundtrip.rows[0].diff_summary.omitted_hunk_count, 99);
    assert!(roundtrip.rows[0].diff_summary.full_source_redacted);

    let mut missing = value;
    remove_required_field::<ProposalLedgerProjection>(&mut missing, "schema_version");
}

#[test]
fn dto_contracts_context_manifest_projection_metadata_only_and_risk_visible() {
    let proposal = WorkspaceProposal {
        proposal_id: devil_protocol::ProposalId(700),
        principal: PrincipalId("principal-1".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(901),
        payload: ProposalPayload::Batch(batch_payload()),
        preconditions: preconditions(),
        preview: PreviewSummary {
            summary: "batch preview".to_string(),
            details: vec!["metadata-only preview".to_string()],
        },
        expires_at: Some(TimestampMillis(2000)),
        created_at: TimestampMillis(1000),
    };
    let mut manifest = context_manifest_from_proposal(
        &proposal,
        "manifest:p5-context",
        Some(WorkspaceTrustState::Trusted),
        ProposalPrivacyLabel::WorkspaceMetadata,
        ProposalRiskLabel::Medium,
        TimestampMillis(1800),
        1,
    );

    let descriptor = SemanticFabricDescriptorReference {
        source_kind: SemanticMetadataSourceKind::DescriptorOnly,
        snapshot_id: Some(SnapshotId(66)),
        content_hash: fingerprint("content"),
        byte_len: Some(1234),
        ranges: vec![ByteRange::new(10, 14)],
        chunks: Vec::new(),
        schema_version: 1,
    };
    let freshness_key = SemanticMetadataFreshnessKey {
        workspace_id: WorkspaceId(11),
        file_id: FileId(33),
        language_id: LanguageId("rust".to_string()),
        snapshot_id: Some(SnapshotId(66)),
        file_content_version: FileContentVersion(44),
        workspace_generation: WorkspaceGeneration(77),
        content_hash: fingerprint("content"),
        grammar_version: Some(SemanticGrammarVersion("grammar-v1".to_string())),
        model_version: Some(SemanticModelVersion("model-v1".to_string())),
        parser_version: "parser-v1".to_string(),
        privacy_scope: SemanticPrivacyScope::Workspace,
        descriptor: SemanticMetadataDescriptorIdentity {
            source_kind: descriptor.source_kind,
            snapshot_id: descriptor.snapshot_id,
            content_hash: descriptor.content_hash.clone(),
            byte_len: descriptor.byte_len,
            ranges: descriptor.ranges.clone(),
            chunks: Vec::new(),
            schema_version: 1,
        },
        schema_version: 1,
    };
    let job = SemanticFabricJobRequest {
        job_id: "semantic-fabric:context".to_string(),
        source_kind: SemanticFabricWorkSourceKind::LspDtoMetadata,
        trigger: SemanticFabricSchedulingTrigger::LspEnrichment,
        workspace_id: WorkspaceId(11),
        file_id: FileId(33),
        language_id: LanguageId("rust".to_string()),
        file_identity: SemanticFileFingerprintIdentity {
            workspace_id: WorkspaceId(11),
            file_id: FileId(33),
            canonical_path: CanonicalPath("C:/repo/src/main.rs".to_string()),
            file_content_version: FileContentVersion(44),
            workspace_generation: WorkspaceGeneration(77),
            content_hash: fingerprint("content"),
            disk_fingerprint: Some(fingerprint("disk")),
            byte_len: Some(1234),
            modified_at: Some(TimestampMillis(9876)),
            privacy_scope: SemanticPrivacyScope::Workspace,
            schema_version: 1,
        },
        expected_freshness_key: freshness_key,
        persisted_freshness_key: None,
        descriptor,
        privacy: SemanticFabricPrivacyLabel {
            privacy_scope: SemanticPrivacyScope::Workspace,
            metadata_only: true,
            redaction: RedactionHint::MetadataOnly,
            schema_version: 1,
        },
        dependency_hints: Vec::new(),
        cancellation: SemanticCancellationToken {
            token_id: cancellation_token_id(),
            workspace_id: WorkspaceId(11),
            file_id: Some(FileId(33)),
            snapshot_id: Some(SnapshotId(66)),
            content_hash: Some(fingerprint("content")),
            workspace_generation: Some(WorkspaceGeneration(77)),
            privacy_scope: SemanticPrivacyScope::Workspace,
            reason: None,
            issued_at: TimestampMillis(1700),
            expires_at: None,
            schema_version: 1,
        },
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        schema_version: 1,
    };
    manifest
        .items
        .push(context_manifest_item_from_semantic_fabric_job(&job, 0, 1));

    let mut lsp_summary = lsp_diagnostic_summary();
    lsp_summary.freshness = SemanticFreshnessState::Stale;
    lsp_summary.content_hash = None;
    manifest
        .items
        .push(context_manifest_item_from_lsp_diagnostic_summary(
            &lsp_summary,
            0,
            1,
        ));
    manifest.permissions.push(ContextManifestPermissionSummary {
        kind: ContextManifestPermissionKind::ModelProvider,
        capability: CapabilityId("model.route.remote".to_string()),
        principal: Some(PrincipalId("principal-1".to_string())),
        decision_id: None,
        granted: false,
        privacy_scope: SemanticPrivacyScope::Workspace,
        egress: ContextManifestEgressStatus::RemoteApprovalRequired,
        risk_label: ProposalRiskLabel::High,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    });
    manifest.stale_or_missing_metadata_risk_present = manifest.items.iter().any(|item| {
        matches!(
            item.risk_label,
            ProposalRiskLabel::Medium | ProposalRiskLabel::High | ProposalRiskLabel::Unknown
        )
    });

    let projection = ContextManifestProjection {
        manifest,
        selected_item_id: Some("semantic-job:0:semantic-fabric:context".to_string()),
        generated_at: TimestampMillis(1800),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let value = serde_json::to_value(&projection).expect("serialize context manifest projection");
    assert_eq!(value["manifest"]["purpose"], "ProposalReview");
    let permissions = value["manifest"]["permissions"]
        .as_array()
        .expect("permissions must be array");
    assert!(
        permissions
            .iter()
            .any(|permission| permission["egress"] == "RemoteApprovalRequired")
    );
    assert_eq!(
        value["manifest"]["stale_or_missing_metadata_risk_present"],
        true
    );
    assert!(value["manifest"]["items"].as_array().unwrap().len() >= 4);

    let serialized = serde_json::to_string(&value).expect("stringify context manifest");
    assert!(!serialized.contains("replacement\":\"main"));
    assert!(!serialized.contains("fn main"));
    assert!(!serialized.contains("raw prompt"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("terminal output"));
    assert!(!serialized.contains("provider_payload"));

    let roundtrip: ContextManifestProjection =
        serde_json::from_value(value.clone()).expect("deserialize context manifest projection");
    assert!(roundtrip.manifest.stale_or_missing_metadata_risk_present);
    assert!(
        roundtrip
            .manifest
            .permissions
            .iter()
            .any(|permission| permission.kind == ContextManifestPermissionKind::ModelProvider)
    );

    let mut missing = value;
    remove_required_field::<ContextManifestProjection>(&mut missing, "schema_version");
}

#[test]
fn dto_contracts_context_manifest_missing_preconditions_are_manifest_risk() {
    let summary = ContextManifestPreconditionSummary::from_preconditions(&empty_preconditions(), 1);
    assert_eq!(summary.risk_label, ProposalRiskLabel::High);
    assert!(!summary.core_preconditions_present);
    assert!(
        summary
            .risk_reasons
            .contains(&"missing.snapshot_id".to_string())
    );
    assert!(
        summary
            .risk_reasons
            .contains(&"missing.expected_fingerprint".to_string())
    );

    let freshness = ContextManifestFreshnessSummary::missing(1);
    assert_eq!(freshness.risk_label, ProposalRiskLabel::High);
    assert!(!freshness.freshness_key_present);
    assert!(
        freshness
            .risk_reasons
            .contains(&"missing.freshness_key".to_string())
    );
}

#[test]
fn dto_contracts_context_manifest_does_not_activate_runtime_requests() {
    let manifest = ContextManifestProjection {
        manifest: ContextManifestRecord {
            manifest_id: "manifest:no-runtime".to_string(),
            workspace_id: Some(WorkspaceId(11)),
            proposal_id: None,
            purpose: ContextManifestPurpose::TrustReview,
            workspace_trust_state: Some(WorkspaceTrustState::Trusted),
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: ProposalRiskLabel::Informational,
            egress: ContextManifestEgressStatus::LocalOnly,
            items: vec![context_manifest_item_from_lsp_supervision_event(
                &LspSupervisionEvent {
                    event_id: EventId(
                        Uuid::parse_str("99999999-9999-9999-9999-999999999999").unwrap(),
                    ),
                    sequence: EventSequence(5),
                    kind: LspSupervisionEventKind::LaunchRefused,
                    identity: lsp_server_identity(),
                    lifecycle_state: LspSupervisionLifecycleState::LaunchDeferred,
                    health_state: LspHealthState::Unavailable,
                    request: Some(lsp_request_correlation()),
                    restart_backoff: None,
                    capabilities: Vec::new(),
                    diagnostic_summaries: Vec::new(),
                    diagnostics: Vec::new(),
                    correlation_id: CorrelationId(901),
                    causality_id: causality_id(),
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                    schema_version: 1,
                },
                1,
            )],
            permissions: Vec::new(),
            omitted_item_count: 0,
            stale_or_missing_metadata_risk_present: false,
            generated_at: TimestampMillis(1800),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        selected_item_id: None,
        generated_at: TimestampMillis(1800),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let serialized = serde_json::to_string(&manifest).expect("serialize no-runtime manifest");
    assert!(!serialized.contains("RegisterServer"));
    assert!(!serialized.contains("OpenDocument"));
    assert!(!serialized.contains("ProviderRequest"));
    assert!(!serialized.contains("std::process"));
    assert!(!serialized.contains("thread::spawn"));
}

#[test]
fn dto_contracts_privacy_inspector_serializes_metadata_only_and_redacted() {
    let projection = ContextManifestProjection {
        manifest: ContextManifestRecord {
            manifest_id: "manifest:privacy".to_string(),
            workspace_id: Some(WorkspaceId(11)),
            proposal_id: Some(ProposalId(700)),
            purpose: ContextManifestPurpose::TrustReview,
            workspace_trust_state: Some(WorkspaceTrustState::Trusted),
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: ProposalRiskLabel::High,
            egress: ContextManifestEgressStatus::RemoteApprovalRequired,
            items: vec![ContextManifestItem {
                item_id: "context:semantic:0".to_string(),
                kind: ContextManifestItemKind::SemanticRecord,
                inclusion: ContextManifestInclusionState::Redacted,
                workspace_id: Some(WorkspaceId(11)),
                file_id: Some(FileId(33)),
                buffer_id: Some(BufferId(22)),
                proposal_id: Some(ProposalId(700)),
                target_id: Some("target:0".to_string()),
                path: Some(CanonicalPath("C:/repo/src/main.rs".to_string())),
                ranges: vec![ByteRange::new(10, 14)],
                counts: vec![ContextManifestItemCount {
                    label: "symbols".to_string(),
                    count: 3,
                }],
                hashes: vec![fingerprint("semantic-record")],
                privacy_scope: Some(SemanticPrivacyScope::Workspace),
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                risk_label: ProposalRiskLabel::Medium,
                egress: ContextManifestEgressStatus::LocalOnly,
                freshness: None,
                preconditions: None,
                labels: vec!["semantic.metadata".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            permissions: vec![ContextManifestPermissionSummary {
                kind: ContextManifestPermissionKind::ModelProvider,
                capability: CapabilityId("provider.invoke".to_string()),
                principal: Some(PrincipalId("principal-1".to_string())),
                decision_id: None,
                granted: false,
                privacy_scope: SemanticPrivacyScope::Workspace,
                egress: ContextManifestEgressStatus::RemoteApprovalRequired,
                risk_label: ProposalRiskLabel::High,
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            }],
            omitted_item_count: 0,
            stale_or_missing_metadata_risk_present: false,
            generated_at: TimestampMillis(1800),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        selected_item_id: None,
        generated_at: TimestampMillis(1800),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let inspector = privacy_inspector_from_context_manifest_projection(
        &projection,
        "privacy:projection",
        TimestampMillis(1801),
        1,
    );
    assert_eq!(inspector.records.len(), 2);
    assert_eq!(inspector.denied_record_count, 1);
    assert_eq!(inspector.external_egress_record_count, 1);
    assert!(inspector.refusal.is_some());

    let value = serde_json::to_value(&inspector).expect("serialize privacy inspector");
    assert_eq!(value["records"][0]["ranges"][0]["start"], 10);
    assert_eq!(value["records"][0]["hashes"][0]["value"], "semantic-record");
    assert_eq!(value["records"][1]["permission_label"], "provider.invoke");

    let serialized = serde_json::to_string(&value).expect("stringify privacy inspector");
    assert!(!serialized.contains("fn main"));
    assert!(!serialized.contains("raw prompt"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("provider_payload"));
    assert!(!serialized.contains("terminal output"));
    assert!(!serialized.contains("replacement\":\""));

    let roundtrip: PrivacyInspectorProjection =
        serde_json::from_value(value.clone()).expect("deserialize privacy inspector");
    assert_eq!(
        roundtrip.refusal.unwrap().reason_code,
        "privacy.scope.denied"
    );

    let mut missing = value;
    remove_required_field::<PrivacyInspectorProjection>(&mut missing, "schema_version");
}

#[test]
fn dto_contracts_permission_budgets_deny_deplete_and_refuse_with_metadata_only_actions() {
    let budget = PermissionBudgetContract {
        budget_id: "budget:provider".to_string(),
        action_class: PermissionBudgetActionClass::InvokeProvider,
        capability: Some(CapabilityId("provider.invoke".to_string())),
        state: PermissionBudgetState::Allowed,
        privacy_scope: SemanticPrivacyScope::Workspace,
        usage: PermissionBudgetUsageSummary {
            unit_label: "calls".to_string(),
            used: 1,
            ceiling: Some(2),
            remaining: Some(1),
            attempted: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        reset_policy_label: PermissionBudgetResetPolicyLabel::Session,
        consent_requirement_label: PermissionBudgetConsentRequirementLabel::NotRequired,
        risk_label: ProposalRiskLabel::High,
        reasons: vec!["provider.route.metadata".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let action = PermissionBudgetActionSummary {
        action_id: "action:provider:0".to_string(),
        action_class: PermissionBudgetActionClass::InvokeProvider,
        capability: Some(CapabilityId("provider.invoke".to_string())),
        workspace_id: Some(WorkspaceId(11)),
        proposal_id: Some(ProposalId(700)),
        target_id: None,
        privacy_scope: SemanticPrivacyScope::Workspace,
        egress: ContextManifestEgressStatus::RemoteApprovalRequired,
        estimated_units: 1,
        ranges: Vec::new(),
        counts: vec![ContextManifestItemCount {
            label: "context_items".to_string(),
            count: 2,
        }],
        hashes: vec![fingerprint("context-manifest")],
        labels: vec!["provider.route.metadata".to_string()],
        risk_label: ProposalRiskLabel::High,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let allowed = evaluate_permission_budget(&budget, action.clone(), "eval:allowed", 1);
    assert!(allowed.allowed);
    assert_eq!(allowed.usage_after.used, 2);
    assert_eq!(allowed.usage_after.remaining, Some(0));

    let depleted = evaluate_permission_budget(&budget, action.clone(), "eval:depleted", 1);
    let depleted_again = evaluate_permission_budget(
        &PermissionBudgetContract {
            usage: depleted.usage_after.clone(),
            ..budget.clone()
        },
        action.clone(),
        "eval:depleted-again",
        1,
    );
    assert!(!depleted_again.allowed);
    assert_eq!(
        depleted_again.disposition,
        PermissionBudgetEvaluationDisposition::RefusedDepleted
    );
    assert_eq!(
        depleted_again.refusal.as_ref().unwrap().reason_code,
        "budget.depleted"
    );

    let denied = evaluate_permission_budget(
        &PermissionBudgetContract {
            state: PermissionBudgetState::Denied,
            ..budget.clone()
        },
        action.clone(),
        "eval:denied",
        1,
    );
    assert!(!denied.allowed);
    assert_eq!(
        denied.disposition,
        PermissionBudgetEvaluationDisposition::RefusedDenied
    );

    let privacy_refused = evaluate_permission_budget(
        &budget,
        PermissionBudgetActionSummary {
            privacy_scope: SemanticPrivacyScope::Redacted,
            egress: ContextManifestEgressStatus::RemoteDenied,
            ..action
        },
        "eval:privacy-refused",
        1,
    );
    assert!(!privacy_refused.allowed);
    assert_eq!(
        privacy_refused.disposition,
        PermissionBudgetEvaluationDisposition::RefusedPrivacyScope
    );
    assert_eq!(
        privacy_refused.refusal.as_ref().unwrap().reason_code,
        "privacy.scope.denied"
    );

    let projection = permission_budget_projection_from_contracts(
        "budgets:trust",
        vec![budget],
        vec![allowed, depleted_again, denied, privacy_refused],
        TimestampMillis(1802),
        1,
    );
    assert_eq!(projection.refused_evaluation_count, 3);
    let serialized = serde_json::to_string(&projection).expect("serialize budget projection");
    assert!(serialized.contains("RefusedPrivacyScope"));
    assert!(!serialized.contains("provider_payload"));
    assert!(!serialized.contains("raw prompt"));
    assert!(!serialized.contains("terminal output"));
    assert!(!serialized.contains("std::process"));
}

#[test]
fn dto_contracts_privacy_and_budget_contracts_do_not_encode_runtime_activation() {
    let permission = ContextManifestPermissionSummary {
        kind: ContextManifestPermissionKind::Tool,
        capability: CapabilityId("tool.local.metadata".to_string()),
        principal: None,
        decision_id: None,
        granted: true,
        privacy_scope: SemanticPrivacyScope::MetadataOnly,
        egress: ContextManifestEgressStatus::LocalOnly,
        risk_label: ProposalRiskLabel::Low,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let action = permission_budget_action_from_permission_summary(
        &permission,
        "action:tool:metadata",
        PermissionBudgetActionClass::InvokeLocalTool,
        Some(WorkspaceId(11)),
        None,
        1,
    );
    let budget = PermissionBudgetContract {
        budget_id: "budget:tool".to_string(),
        action_class: PermissionBudgetActionClass::InvokeLocalTool,
        capability: Some(CapabilityId("tool.local.metadata".to_string())),
        state: PermissionBudgetState::Allowed,
        privacy_scope: SemanticPrivacyScope::MetadataOnly,
        usage: PermissionBudgetUsageSummary {
            unit_label: "calls".to_string(),
            used: 0,
            ceiling: Some(5),
            remaining: Some(5),
            attempted: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        reset_policy_label: PermissionBudgetResetPolicyLabel::ManualApproval,
        consent_requirement_label: PermissionBudgetConsentRequirementLabel::NotRequired,
        risk_label: ProposalRiskLabel::Low,
        reasons: vec!["tool.metadata_only".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let evaluation = evaluate_permission_budget(&budget, action, "eval:tool", 1);
    let serialized = serde_json::to_string(&(budget, evaluation)).expect("serialize contracts");
    assert!(serialized.contains("MetadataOnly"));
    assert!(!serialized.contains("ProviderRequest"));
    assert!(!serialized.contains("RegisterServer"));
    assert!(!serialized.contains("OpenDocument"));
    assert!(!serialized.contains("TerminalRequest"));
    assert!(!serialized.contains("std::process"));
    assert!(!serialized.contains("thread::spawn"));
}

#[test]
fn dto_contracts_approval_checklist_ready_state_is_metadata_only_and_non_mutating() {
    let proposal = WorkspaceProposal {
        proposal_id: devil_protocol::ProposalId(700),
        principal: PrincipalId("principal-1".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(901),
        payload: ProposalPayload::Batch(batch_payload()),
        preconditions: preconditions(),
        preview: PreviewSummary {
            summary: "batch preview".to_string(),
            details: vec!["metadata-only preview".to_string()],
        },
        expires_at: Some(TimestampMillis(2000)),
        created_at: TimestampMillis(1000),
    };
    let mut manifest = context_manifest_from_proposal(
        &proposal,
        "manifest:approval-ready",
        Some(WorkspaceTrustState::Trusted),
        ProposalPrivacyLabel::WorkspaceMetadata,
        ProposalRiskLabel::Low,
        TimestampMillis(1800),
        1,
    );
    manifest.permissions[0].granted = true;
    let manifest_projection = ContextManifestProjection {
        manifest,
        selected_item_id: None,
        generated_at: TimestampMillis(1800),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let privacy = privacy_inspector_from_context_manifest_projection(
        &manifest_projection,
        "privacy:approval-ready",
        TimestampMillis(1801),
        1,
    );
    let budget = PermissionBudgetContract {
        budget_id: "budget:apply".to_string(),
        action_class: PermissionBudgetActionClass::ApplyApprovedProposal,
        capability: Some(CapabilityId("fs.write".to_string())),
        state: PermissionBudgetState::Allowed,
        privacy_scope: SemanticPrivacyScope::MetadataOnly,
        usage: PermissionBudgetUsageSummary {
            unit_label: "proposals".to_string(),
            used: 0,
            ceiling: Some(5),
            remaining: Some(5),
            attempted: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        reset_policy_label: PermissionBudgetResetPolicyLabel::ManualApproval,
        consent_requirement_label: PermissionBudgetConsentRequirementLabel::NotRequired,
        risk_label: ProposalRiskLabel::Low,
        reasons: vec!["approval.metadata_only".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let action = PermissionBudgetActionSummary {
        action_id: "action:apply:700".to_string(),
        action_class: PermissionBudgetActionClass::ApplyApprovedProposal,
        capability: Some(CapabilityId("fs.write".to_string())),
        workspace_id: Some(WorkspaceId(11)),
        proposal_id: Some(devil_protocol::ProposalId(700)),
        target_id: None,
        privacy_scope: SemanticPrivacyScope::MetadataOnly,
        egress: ContextManifestEgressStatus::LocalOnly,
        estimated_units: 1,
        ranges: Vec::new(),
        counts: vec![ContextManifestItemCount {
            label: "targets".to_string(),
            count: 2,
        }],
        hashes: vec![fingerprint("approval-context")],
        labels: vec!["apply.approved_proposal".to_string()],
        risk_label: ProposalRiskLabel::Low,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let evaluation = evaluate_permission_budget(&budget, action, "eval:apply:700", 1);
    let budgets = permission_budget_projection_from_contracts(
        "budgets:approval-ready",
        vec![budget],
        vec![evaluation],
        TimestampMillis(1802),
        1,
    );
    let rollback = checkpoint_rollback_projection_from_proposal(
        "checkpoint-rollback:700",
        &proposal,
        ProposalLifecycleState::Previewed,
        None,
        CheckpointRollbackAuditStatus::Available,
        Some(causality_id()),
        TimestampMillis(1803),
        1,
    );
    let checklist = approval_checklist_from_trust_projections(
        "approval-checklist:700",
        &proposal,
        ProposalLifecycleState::Previewed,
        None,
        Some(&manifest_projection),
        Some(&privacy),
        Some(&budgets),
        Some(&rollback),
        true,
        Some(causality_id()),
        TimestampMillis(1804),
        1,
    );

    assert!(checklist.ready_for_approval);
    assert!(checklist.blockers.is_empty());
    assert!(checklist.gates.iter().all(|gate| !matches!(
        gate.status,
        ApprovalChecklistGateStatus::Blocked | ApprovalChecklistGateStatus::Unknown
    )));
    assert_eq!(checklist.lifecycle_state, ProposalLifecycleState::Previewed);

    let value = serde_json::to_value(&checklist).expect("serialize approval checklist");
    assert_eq!(value["ready_for_approval"], true);
    assert_eq!(value["payload_kind"], "Batch");
    let serialized = serde_json::to_string(&value).expect("stringify approval checklist");
    assert!(serialized.contains("MetadataOnly"));
    assert!(!serialized.contains("fn main"));
    assert!(!serialized.contains("raw prompt"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("provider_payload"));
    assert!(!serialized.contains("terminal output"));
    assert!(!serialized.contains("std::process"));
    assert!(!serialized.contains("ApplyProposal"));

    let roundtrip: ProposalApprovalChecklistProjection =
        serde_json::from_value(value.clone()).expect("deserialize approval checklist");
    assert!(roundtrip.ready_for_approval);
    let mut missing = value;
    remove_required_field::<ProposalApprovalChecklistProjection>(&mut missing, "schema_version");
}

#[test]
fn dto_contracts_approval_checklist_surfaces_each_blocker_class() {
    let mut proposal = proposal_for_lsp();
    proposal.preconditions.expected_fingerprint = None;
    proposal.expires_at = Some(TimestampMillis(1700));
    let mut privacy = PrivacyInspectorProjection {
        inspector_id: "privacy:blockers".to_string(),
        manifest_id: Some("manifest:blockers".to_string()),
        workspace_id: Some(WorkspaceId(11)),
        proposal_id: Some(proposal.proposal_id),
        records: Vec::new(),
        denied_record_count: 1,
        redacted_record_count: 1,
        external_egress_record_count: 0,
        high_risk_record_count: 1,
        refusal: None,
        generated_at: TimestampMillis(1800),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    privacy.refusal = Some(PrivacyInspectorRefusal {
        reason_code: "privacy.scope.denied".to_string(),
        label: "Privacy scope denied".to_string(),
        privacy_scope: Some(SemanticPrivacyScope::Redacted),
        capability: Some(CapabilityId("provider.invoke".to_string())),
        budget_id: None,
        risk_label: ProposalRiskLabel::High,
        reasons: vec!["privacy.denied".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    });
    let depleted_budget = PermissionBudgetContract {
        budget_id: "budget:depleted".to_string(),
        action_class: PermissionBudgetActionClass::ApplyApprovedProposal,
        capability: Some(CapabilityId("fs.write".to_string())),
        state: PermissionBudgetState::Depleted,
        privacy_scope: SemanticPrivacyScope::MetadataOnly,
        usage: PermissionBudgetUsageSummary {
            unit_label: "proposals".to_string(),
            used: 1,
            ceiling: Some(1),
            remaining: Some(0),
            attempted: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        reset_policy_label: PermissionBudgetResetPolicyLabel::ManualApproval,
        consent_requirement_label: PermissionBudgetConsentRequirementLabel::NotRequired,
        risk_label: ProposalRiskLabel::High,
        reasons: vec!["budget.depleted".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let budgets = permission_budget_projection_from_contracts(
        "budgets:blockers",
        vec![depleted_budget],
        Vec::new(),
        TimestampMillis(1800),
        1,
    );
    let mut rollback = checkpoint_rollback_projection_from_proposal(
        "checkpoint-rollback:blockers",
        &proposal,
        ProposalLifecycleState::Created,
        None,
        CheckpointRollbackAuditStatus::Missing,
        Some(causality_id()),
        TimestampMillis(1800),
        1,
    );
    rollback.rollback.availability = ProposalRollbackAvailability::Unavailable;
    let checklist = approval_checklist_from_trust_projections(
        "approval-checklist:blockers",
        &proposal,
        ProposalLifecycleState::Created,
        None,
        None,
        Some(&privacy),
        Some(&budgets),
        Some(&rollback),
        false,
        Some(causality_id()),
        TimestampMillis(1800),
        1,
    );

    assert!(!checklist.ready_for_approval);
    let reasons = checklist
        .blockers
        .iter()
        .map(|reason| reason.reason_code.as_str())
        .collect::<Vec<_>>();
    assert!(reasons.contains(&"context_manifest.missing"));
    assert!(reasons.contains(&"privacy.scope.denied"));
    assert!(reasons.contains(&"budget.depleted"));
    assert!(reasons.contains(&"proposal.lifecycle_not_previewed"));
    assert!(reasons.contains(&"missing.expected_fingerprint"));
    assert!(reasons.contains(&"proposal.expired"));
    assert!(reasons.contains(&"audit_before_success.missing"));
    assert!(reasons.contains(&"rollback.unavailable"));
    assert!(
        checklist
            .explicit_denial_reasons
            .iter()
            .any(|reason| reason == "privacy.scope.denied")
    );
}

#[test]
fn dto_contracts_checkpoint_rollback_projection_exposes_only_metadata() {
    let proposal = WorkspaceProposal {
        proposal_id: devil_protocol::ProposalId(700),
        principal: PrincipalId("principal-1".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(901),
        payload: ProposalPayload::Batch(batch_payload()),
        preconditions: preconditions(),
        preview: PreviewSummary {
            summary: "batch preview".to_string(),
            details: vec!["metadata-only preview".to_string()],
        },
        expires_at: Some(TimestampMillis(2000)),
        created_at: TimestampMillis(1000),
    };
    let projection = checkpoint_rollback_projection_from_proposal(
        "checkpoint-rollback:700",
        &proposal,
        ProposalLifecycleState::Previewed,
        None,
        CheckpointRollbackAuditStatus::Available,
        Some(causality_id()),
        TimestampMillis(1800),
        1,
    );

    assert!(projection.checkpoint.available);
    assert_eq!(
        projection.rollback.availability,
        ProposalRollbackAvailability::Available
    );
    assert_eq!(projection.rollback.rollback_step_count, 2);
    assert_eq!(projection.targets.len(), 2);
    assert!(
        projection
            .targets
            .iter()
            .all(|target| !target.labels.is_empty())
    );
    assert!(
        projection
            .targets
            .iter()
            .all(|target| target.hashes.iter().all(|hash| hash.value == "expected"))
    );

    let value = serde_json::to_value(&projection).expect("serialize checkpoint rollback");
    assert_eq!(value["rollback"]["availability"], "Available");
    assert_eq!(value["targets"][0]["ranges"][0]["start"], 10);
    let serialized = serde_json::to_string(&value).expect("stringify checkpoint rollback");
    assert!(serialized.contains("checkpoint:proposal:700:metadata"));
    assert!(serialized.contains("expected_file_content_version"));
    assert!(!serialized.contains("fn main"));
    assert!(!serialized.contains("raw prompt"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("provider_payload"));
    assert!(!serialized.contains("terminal output"));
    assert!(!serialized.contains("replacement\":"));
    assert!(!serialized.contains("C:/repo"));

    let roundtrip: CheckpointRollbackProjection =
        serde_json::from_value(value.clone()).expect("deserialize checkpoint rollback");
    assert_eq!(roundtrip.rollback.rollback_step_count, 2);
    let mut missing = value;
    remove_required_field::<CheckpointRollbackProjection>(&mut missing, "schema_version");
}

#[test]
fn dto_contracts_p5_3_contracts_do_not_encode_runtime_activation() {
    let proposal = proposal_for_lsp();
    let rollback = checkpoint_rollback_projection_from_proposal(
        "checkpoint-rollback:no-runtime",
        &proposal,
        ProposalLifecycleState::Previewed,
        None,
        CheckpointRollbackAuditStatus::Pending,
        Some(causality_id()),
        TimestampMillis(1800),
        1,
    );
    let checklist = approval_checklist_from_trust_projections(
        "approval-checklist:no-runtime",
        &proposal,
        ProposalLifecycleState::Previewed,
        None,
        None,
        None,
        None,
        Some(&rollback),
        false,
        Some(causality_id()),
        TimestampMillis(1800),
        1,
    );
    let serialized = serde_json::to_string(&(rollback, checklist)).expect("serialize p5.3");
    assert!(serialized.contains("MetadataOnly"));
    assert!(!serialized.contains("ProviderRequest"));
    assert!(!serialized.contains("RegisterServer"));
    assert!(!serialized.contains("OpenDocument"));
    assert!(!serialized.contains("TerminalRequest"));
    assert!(!serialized.contains("std::process"));
    assert!(!serialized.contains("thread::spawn"));
    assert!(!serialized.contains("ApplyProposal"));
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
    let collaboration_audit = CollaborationAuditRecord {
        session_id: CollaborationSessionId(1001),
        operation_id: Some(CollaborationOperationId(3001)),
        proposal_id: Some(ProposalId(700)),
        event_sequence: EventSequence(8),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        retention_label: RetentionLabel::Audit,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        metadata_summary: "operation_count=1 byte_count=42".to_string(),
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
    let semantic_chunk = SemanticMetadataChunkReference {
        snapshot_id: SnapshotId(66),
        chunk_index: 0,
        byte_range: ByteRange::new(0, 128),
        line_range: LineIndexRange { start: 0, end: 8 },
        byte_len: 128,
        chunk_hash: chunk_hash("semantic-chunk"),
        lease_present: false,
        schema_version: 1,
    };
    let semantic_descriptor = SemanticMetadataDescriptorIdentity {
        source_kind: SemanticMetadataSourceKind::DescriptorOnly,
        snapshot_id: Some(SnapshotId(66)),
        content_hash: fingerprint("semantic-content"),
        byte_len: Some(128),
        ranges: vec![ByteRange::new(0, 128)],
        chunks: vec![semantic_chunk],
        schema_version: 1,
    };
    let semantic_freshness_key = SemanticMetadataFreshnessKey {
        workspace_id: WorkspaceId(11),
        file_id: FileId(33),
        language_id: LanguageId("rust".to_string()),
        snapshot_id: Some(SnapshotId(66)),
        file_content_version: FileContentVersion(44),
        workspace_generation: WorkspaceGeneration(77),
        content_hash: fingerprint("semantic-content"),
        grammar_version: Some(SemanticGrammarVersion("grammar-v1".to_string())),
        model_version: Some(SemanticModelVersion("metadata-model-v1".to_string())),
        parser_version: "parser-v1".to_string(),
        privacy_scope: SemanticPrivacyScope::Workspace,
        descriptor: semantic_descriptor,
        schema_version: 1,
    };
    let semantic_record = SemanticMetadataRecord {
        record_id: SemanticRecordId("semantic-record-33".to_string()),
        workspace_id: WorkspaceId(11),
        file_id: FileId(33),
        language_id: LanguageId("rust".to_string()),
        freshness_key: semantic_freshness_key.clone(),
        file_identity: SemanticFileFingerprintIdentity {
            workspace_id: WorkspaceId(11),
            file_id: FileId(33),
            canonical_path: CanonicalPath("C:/repo/src/main.rs".to_string()),
            file_content_version: FileContentVersion(44),
            workspace_generation: WorkspaceGeneration(77),
            content_hash: fingerprint("semantic-content"),
            disk_fingerprint: Some(fingerprint("file")),
            byte_len: Some(128),
            modified_at: Some(TimestampMillis(9876)),
            privacy_scope: SemanticPrivacyScope::Workspace,
            schema_version: 1,
        },
        provenance: SemanticRecordProvenance {
            source: SemanticRecordSource::Lexical,
            server_id: None,
            extraction_version: "parser-v1".to_string(),
            confidence_basis_points: 10_000,
        },
        symbols: vec![SemanticMetadataSymbolRecord {
            symbol_id: SemanticSymbolId("symbol-33".to_string()),
            symbol_name_hash: fingerprint("symbol-name"),
            kind_hash: fingerprint("symbol-kind"),
            declaration_range: Some(protocol_range()),
            reference_ranges: Vec::new(),
            schema_version: 1,
        }],
        graph_records: Vec::new(),
        diagnostic_summaries: Vec::new(),
        freshness_state: SemanticFreshnessState::Fresh,
        persisted_at: TimestampMillis(1800),
        schema_version: 1,
    };
    let semantic_tombstone = SemanticMetadataTombstone {
        workspace_id: WorkspaceId(11),
        file_id: Some(FileId(33)),
        freshness_key: Some(semantic_freshness_key.clone()),
        reason: SemanticMetadataTombstoneReason::PrivacyScopeRevoked,
        observed_at: TimestampMillis(1900),
        schema_version: 1,
    };
    let semantic_query = SemanticMetadataQuery {
        workspace_id: WorkspaceId(11),
        file_ids: vec![FileId(33)],
        language_ids: vec![LanguageId("rust".to_string())],
        privacy_scope: SemanticPrivacyScope::Workspace,
        freshness_key: Some(semantic_freshness_key.clone()),
        include_stale: false,
        schema_version: 1,
    };

    let requests = vec![
        StorageRepositoryRequest::SaveFileMetadata(metadata.clone()),
        StorageRepositoryRequest::SaveTrustRecord(trust.clone()),
        StorageRepositoryRequest::SaveProposalAuditRecord(audit.clone()),
        StorageRepositoryRequest::SaveEventMetadata(event_metadata.clone()),
        StorageRepositoryRequest::SaveCollaborationAuditRecord(collaboration_audit.clone()),
        StorageRepositoryRequest::SaveSemanticMetadata(SemanticMetadataBatch {
            records: vec![semantic_record.clone()],
            tombstones: Vec::new(),
            correlation_id: CorrelationId(901),
            causality_id: causality_id(),
            schema_version: 1,
        }),
        StorageRepositoryRequest::TombstoneSemanticMetadata(semantic_tombstone.clone()),
        StorageRepositoryRequest::ReadSessionRecord {
            session_id: "session-1".to_string(),
        },
        StorageRepositoryRequest::ReadTrustRecord {
            workspace_id: WorkspaceId(11),
            principal_id: PrincipalId("principal-1".to_string()),
        },
        StorageRepositoryRequest::ReadProposalAuditRecord(devil_protocol::ProposalId(700)),
        StorageRepositoryRequest::ReadEventMetadata(event_id),
        StorageRepositoryRequest::ReadCollaborationAuditRecord {
            session_id: CollaborationSessionId(1001),
            event_sequence: EventSequence(8),
        },
        StorageRepositoryRequest::ReadSemanticMetadata(semantic_query),
        StorageRepositoryRequest::ReadSemanticMetadataTombstones {
            workspace_id: WorkspaceId(11),
            file_id: Some(FileId(33)),
        },
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
        request_value[5]["SaveSemanticMetadata"]["records"][0]["symbols"][0]["symbol_name_hash"]["value"],
        "symbol-name"
    );
    assert_eq!(
        request_value[6]["TombstoneSemanticMetadata"]["reason"],
        "PrivacyScopeRevoked"
    );
    assert_eq!(
        request_value[7]["ReadSessionRecord"]["session_id"],
        "session-1"
    );
    assert_eq!(request_value[9]["ReadProposalAuditRecord"], 700);
    assert_eq!(
        request_value[12]["ReadSemanticMetadata"]["include_stale"],
        false
    );

    let responses = vec![
        StorageRepositoryResponse::FileMetadata(Some(metadata)),
        StorageRepositoryResponse::TrustRecord(Some(trust)),
        StorageRepositoryResponse::ProposalAuditRecord(Some(audit)),
        StorageRepositoryResponse::EventMetadata(Some(event_metadata)),
        StorageRepositoryResponse::CollaborationAuditRecord(Box::new(Some(collaboration_audit))),
        StorageRepositoryResponse::SemanticMetadata(SemanticMetadataReadResult {
            records: vec![semantic_record],
            rejected: Vec::new(),
            schema_version: 1,
        }),
        StorageRepositoryResponse::SemanticMetadataTombstones(vec![semantic_tombstone]),
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
    assert_eq!(
        response_value[5]["SemanticMetadata"]["records"][0]["freshness_key"]["parser_version"],
        "parser-v1"
    );
    assert_eq!(
        response_value[6]["SemanticMetadataTombstones"][0]["reason"],
        "PrivacyScopeRevoked"
    );

    let roundtrip: Vec<StorageRepositoryRequest> =
        serde_json::from_value(request_value).expect("deserialize storage requests");
    assert!(matches!(
        roundtrip[9],
        StorageRepositoryRequest::ReadProposalAuditRecord(_)
    ));
    let serialized_semantic = serde_json::to_string(&response_value[5]).expect("semantic json");
    assert!(!serialized_semantic.contains("fn main"));
    assert!(!serialized_semantic.contains("source_body"));
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
            plugin_id: Some(PluginId(42)),
            plugin_host_call_name: Some("createProposal".to_string()),
            plugin_module_hash: Some("sha256:module".to_string()),
            plugin_manifest_id: Some("manifest-42".to_string()),
            plugin_declared_capability_id: Some(CapabilityId("plugin.proposal.create".to_string())),
            plugin_quota_class: Some(PluginQuotaClass::HostCall),
            plugin_sandbox_operation_class: Some(PluginSandboxOperationClass::HostCall),
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
                "plugin_id": 42,
                "plugin_host_call_name": "createProposal",
                "plugin_module_hash": "sha256:module",
                "plugin_manifest_id": "manifest-42",
                "plugin_declared_capability_id": "plugin.proposal.create",
                "plugin_quota_class": "HostCall",
                "plugin_sandbox_operation_class": "HostCall",
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
            assert_eq!(context.plugin_id, Some(PluginId(42)));
            assert!(matches!(
                context.plugin_quota_class,
                Some(PluginQuotaClass::HostCall)
            ));
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
fn dto_contracts_plugin_manifest_requires_abi_trust_capabilities_and_quota_metadata() {
    let manifest = PluginManifest {
        plugin_id: PluginId(7),
        name: "phase5.test".to_string(),
        version: "0.1.0".to_string(),
        schema_version: 1,
        min_abi_version: 1,
        max_abi_version: 1,
        module_hash: "sha256:module".to_string(),
        manifest_id: "manifest:phase5".to_string(),
        trust: PluginTrustMetadata {
            source: PluginTrustSource::ExplicitLocalAllow,
            decision: PluginTrustDecision::ExplicitlyAllowed,
            reason: "test allow".to_string(),
        },
        signature: Some(PluginSignatureMetadata {
            signer: "devil.local".to_string(),
            algorithm: "ed25519".to_string(),
            signature_digest: "sha256:signature".to_string(),
        }),
        activation_events: vec![PluginActivationEvent::OnCommand {
            command: "phase5.run".to_string(),
        }],
        contributions: vec![PluginContribution::Command(PluginCommandDescriptor {
            command_id: "phase5.run".to_string(),
            title: "Phase 5 Run".to_string(),
            required_capability: CapabilityId("plugin.command".to_string()),
        })],
        requested_capabilities: vec![CapabilityId("plugin.command".to_string())],
        storage_namespace: PluginStateNamespace {
            plugin_id: PluginId(7),
            namespace: "state".to_string(),
        },
        quotas: PluginQuotaDeclaration {
            max_fuel: 1000,
            max_wall_time_ms: 50,
            max_memory_pages: 8,
            max_storage_bytes: 4096,
            max_host_calls: 16,
            max_events: 8,
            max_output_bytes: 1024,
        },
    };

    let value = serde_json::to_value(&manifest).expect("serialize plugin manifest");
    assert_eq!(value["plugin_id"], 7);
    assert_eq!(value["min_abi_version"], 1);
    assert_eq!(value["trust"]["decision"], "ExplicitlyAllowed");
    assert_eq!(value["requested_capabilities"][0], "plugin.command");
    assert_eq!(value["quotas"]["max_memory_pages"], 8);
    validate_plugin_manifest(&manifest, 1).expect("valid manifest");

    let mut invalid = manifest;
    invalid.storage_namespace.plugin_id = PluginId(8);
    assert!(validate_plugin_manifest(&invalid, 1).is_err());
}

#[test]
fn dto_contracts_plugin_host_call_and_storage_schemas_are_versioned_and_metadata_only() {
    let host_call = PluginHostCallRequest {
        plugin_id: PluginId(7),
        kind: PluginHostCallKind::CreateProposal,
        host_call_name: "createProposal".to_string(),
        declared_capability: CapabilityId("plugin.proposal.create".to_string()),
        correlation_id: CorrelationId(77),
        causality_id: causality_id(),
        sequence: EventSequence(88),
        metadata_label: "bounded-proposal-output".to_string(),
    };
    validate_plugin_host_call_request(&host_call).expect("host call metadata validates");
    let host_value = serde_json::to_value(&host_call).expect("serialize host call");
    assert_eq!(host_value["kind"], "CreateProposal");
    assert!(!host_value.to_string().contains("source_body"));

    let storage = PluginStorageRecord {
        workspace_id: WorkspaceId(1),
        plugin_id: PluginId(7),
        namespace: PluginStateNamespace {
            plugin_id: PluginId(7),
            namespace: "state".to_string(),
        },
        key: "settings".to_string(),
        value: "metadata-only".to_string(),
        schema_version: 1,
        retention: RetentionLabel::Warm,
        redaction: RedactionHint::MetadataOnly,
        byte_count: 13,
    };
    validate_plugin_storage_record(&storage).expect("plugin storage metadata validates");
    let storage_value = serde_json::to_value(StorageRepositoryRequest::PluginStorage(
        PluginStorageRequest {
            operation: PluginStorageOperation::Put,
            workspace_id: WorkspaceId(1),
            plugin_id: PluginId(7),
            namespace: storage.namespace.clone(),
            key: Some(storage.key.clone()),
            record: Some(storage),
            quota_bytes: 4096,
            correlation_id: CorrelationId(99),
        },
    ))
    .expect("serialize plugin storage request");
    assert_eq!(storage_value["PluginStorage"]["operation"], "Put");
    assert!(!storage_value.to_string().contains("raw_prompt"));
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

fn assisted_ai_provider(provider_class: AssistedAiProviderClass) -> AssistedAiProviderCapability {
    AssistedAiProviderCapability {
        provider_id: "provider:local-redacted".to_string(),
        provider_label: "Local metadata provider".to_string(),
        provider_class,
        supported_operations: vec![
            AssistedAiOperationClass::Explain,
            AssistedAiOperationClass::ProposeEdit,
            AssistedAiOperationClass::StructuredMetadata,
        ],
        model_capability_labels: vec![
            "structured-output".to_string(),
            "proposal-draft".to_string(),
        ],
        tool_capability_labels: vec!["tool-labels-only".to_string()],
        context_window_label: "bounded-context-window".to_string(),
        cost_budget_label: "session-capped".to_string(),
        risk_budget_label: "approval-required".to_string(),
        privacy_retention_label: "metadata-only-no-retention".to_string(),
        byok_support: AssistedAiSupportLabel::Supported,
        local_execution_support: AssistedAiSupportLabel::Supported,
        offline_support: AssistedAiSupportLabel::Supported,
        air_gap_support: AssistedAiSupportLabel::Supported,
        redaction_requirements: vec![
            "payload-redacted".to_string(),
            "sensitive-values-redacted".to_string(),
        ],
        consent_requirements: vec!["workspace-trust".to_string(), "privacy-scope".to_string()],
        availability: AssistedAiProviderAvailabilityState::Available,
        refusal: None,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn assisted_ai_ref(
    reference_id: &str,
    kind: AssistedAiTrustProjectionKind,
) -> AssistedAiTrustProjectionReference {
    AssistedAiTrustProjectionReference {
        reference_id: reference_id.to_string(),
        kind,
        projection_hash: fingerprint(reference_id),
        schema_version: 1,
    }
}

fn assisted_ai_budget_evaluation(
    evaluation_id: &str,
    state: PermissionBudgetState,
    disposition: PermissionBudgetEvaluationDisposition,
    allowed: bool,
) -> PermissionBudgetEvaluation {
    PermissionBudgetEvaluation {
        evaluation_id: evaluation_id.to_string(),
        budget_id: "budget:provider-route".to_string(),
        action: PermissionBudgetActionSummary {
            action_id: "action:assisted-ai-route".to_string(),
            action_class: PermissionBudgetActionClass::InvokeProvider,
            capability: Some(CapabilityId("provider.invoke".to_string())),
            workspace_id: Some(WorkspaceId(11)),
            proposal_id: Some(ProposalId(700)),
            target_id: Some("target-buffer-main".to_string()),
            privacy_scope: SemanticPrivacyScope::Workspace,
            egress: ContextManifestEgressStatus::LocalProvider,
            estimated_units: 1,
            ranges: vec![ByteRange::new(10, 14)],
            counts: vec![ContextManifestItemCount {
                label: "manifest-items".to_string(),
                count: 2,
            }],
            hashes: vec![fingerprint("manifest-hash")],
            labels: vec!["provider-route-metadata".to_string()],
            risk_label: ProposalRiskLabel::Medium,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        disposition,
        state,
        allowed,
        usage_after: PermissionBudgetUsageSummary {
            unit_label: "calls".to_string(),
            used: if allowed { 1 } else { 2 },
            ceiling: Some(2),
            remaining: if allowed { Some(1) } else { Some(0) },
            attempted: 1,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        refusal: None,
        reasons: vec!["provider-route-metadata".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn assisted_ai_boundary() -> AssistedAiConsentBoundary {
    AssistedAiConsentBoundary {
        boundary_id: "boundary:assisted-ai".to_string(),
        workspace_id: Some(WorkspaceId(11)),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        requested_privacy_scope: SemanticPrivacyScope::Workspace,
        privacy_scope_allowed: true,
        consent_state: AssistedAiConsentState::Granted,
        budget_evaluations: vec![assisted_ai_budget_evaluation(
            "eval:allowed",
            PermissionBudgetState::Allowed,
            PermissionBudgetEvaluationDisposition::Allowed,
            true,
        )],
        air_gap_mode: false,
        offline_mode: false,
        local_only_mode: false,
        required_capability: Some(CapabilityId("provider.invoke".to_string())),
        reasons: vec!["boundary.metadata-only".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn assisted_ai_proposal_intent() -> AssistedAiProposalTargetIntent {
    AssistedAiProposalTargetIntent {
        payload_kind: ProposalPayloadKind::TextEdit,
        target_coverage: batch_target_coverage(),
        required_capability: CapabilityId("fs.write".to_string()),
        risk_label: ProposalRiskLabel::Medium,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        labels: vec!["proposal-only".to_string(), "review-required".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn assisted_ai_request_with_decision(
    provider: AssistedAiProviderCapability,
    boundary: AssistedAiConsentBoundary,
    decision: AssistedAiRouteDecision,
) -> AssistedAiRequestContract {
    AssistedAiRequestContract::new(
        "assist:req:1",
        provider,
        AssistedAiOperationClass::ProposeEdit,
        assisted_ai_ref(
            "manifest:p5:context",
            AssistedAiTrustProjectionKind::ContextManifest,
        ),
        assisted_ai_ref(
            "privacy:p5:inspector",
            AssistedAiTrustProjectionKind::PrivacyInspector,
        ),
        assisted_ai_ref(
            "budget:p5:projection",
            AssistedAiTrustProjectionKind::PermissionBudget,
        ),
        boundary
            .budget_evaluations
            .iter()
            .map(|evaluation| {
                AssistedAiPermissionBudgetEvaluationReference::from_evaluation(
                    evaluation,
                    fingerprint(&evaluation.evaluation_id),
                    1,
                )
            })
            .collect(),
        assisted_ai_ref(
            "checklist:p5:approval",
            AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
        ),
        Some(assisted_ai_ref(
            "rollback:p5:checkpoint",
            AssistedAiTrustProjectionKind::CheckpointRollback,
        )),
        CorrelationId(901),
        causality_id(),
        assisted_ai_proposal_intent(),
        decision,
        TimestampMillis(2000),
        1,
    )
    .expect("request contract should validate correlation metadata")
}

fn assisted_ai_output(proposal_id: ProposalId) -> AssistedAiEditProposalOutput {
    AssistedAiEditProposalOutput {
        output_id: "assist:output:1".to_string(),
        request_id: "assist:req:1".to_string(),
        provider_id: "provider:local-redacted".to_string(),
        proposal_id,
        principal: PrincipalId("principal-1".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        payload: ProposalPayload::TextEdit(TextEditProposal {
            file_id: FileId(33),
            edits: EditBatch {
                edits: vec![TextEdit {
                    range: TextRange::byte(10, 14),
                    replacement: "renamed".to_string(),
                }],
            },
        }),
        preconditions: preconditions(),
        preview: PreviewSummary {
            summary: "assisted AI proposal preview".to_string(),
            details: vec!["bounded replacement text is present only inside edit DTOs".to_string()],
        },
        expires_at: Some(TimestampMillis(3000)),
        created_at: TimestampMillis(2000),
        context_manifest: assisted_ai_ref(
            "manifest:p5:context",
            AssistedAiTrustProjectionKind::ContextManifest,
        ),
        approval_checklist: assisted_ai_ref(
            "checklist:p5:approval",
            AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
        ),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn assisted_ai_audit(proposal_id: Option<ProposalId>) -> AssistedAiAuditRecord {
    AssistedAiAuditRecord {
        audit_id: "assist:audit:req-1:1".to_string(),
        provider_capability_id: "provider:local-redacted".to_string(),
        provider_capability_hash: fingerprint("provider-capability-hash"),
        route_decision_id: "assist:route:req-1".to_string(),
        route_decision_hash: fingerprint("route-decision-hash"),
        consent_disposition: Some(AssistedAiConsentState::Granted),
        budget_dispositions: vec![PermissionBudgetEvaluationDisposition::Allowed],
        privacy_disposition: AssistedAiAuditPrivacyDisposition::Allowed,
        request_contract_id: "assist:req:1".to_string(),
        request_contract_hash: fingerprint("request-contract-hash"),
        projection_id: Some("assisted-ai:p6-3".to_string()),
        projection_hash: Some(fingerprint("projection-hash")),
        preview_id: Some("assist:preview:701".to_string()),
        preview_hash: Some(fingerprint("preview-hash")),
        proposal_id,
        outcome_category: AssistedAiAuditOutcomeCategory::ProposalPreviewReady,
        refusal_error_category: None,
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        event_sequence: EventSequence(77),
        risk_labels: vec![ProposalRiskLabel::Medium],
        privacy_labels: vec![ProposalPrivacyLabel::WorkspaceMetadata],
        redaction_state: AssistedAiAuditRedactionState::MetadataOnly,
        runtime_invocation_state: AssistedAiProviderInvocationState::NotEncoded,
        runtime_activation_labels: vec![
            "provider.invocation.not_encoded".to_string(),
            "network.not_encoded".to_string(),
            "tool.disabled".to_string(),
            "agent.disabled".to_string(),
            "terminal.disabled".to_string(),
        ],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn assisted_ai_ledger(proposal_id: ProposalId) -> ProposalLedgerProjection {
    ProposalLedgerProjection {
        rows: vec![ProposalLedgerRow {
            proposal_id,
            workspace_id: Some(WorkspaceId(11)),
            title: "assisted AI reviewable preview".to_string(),
            payload_kind: ProposalPayloadKind::TextEdit,
            lifecycle: ProposalLifecycleStateDisplay {
                state: ProposalLifecycleState::Previewed,
                label: "Previewed".to_string(),
                description: "ready for review".to_string(),
            },
            principal: PrincipalId("principal-1".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            created_at: TimestampMillis(2000),
            updated_at: TimestampMillis(2100),
            expires_at: Some(TimestampMillis(3000)),
            risk_label: ProposalRiskLabel::Medium,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            rollback: ProposalRollbackAvailability::BestEffort,
            target_coverage: batch_target_coverage(),
            context_manifest: ProposalContextManifestSummary {
                manifest_id: "manifest:p5:context".to_string(),
                category_count: 1,
                total_item_count: 1,
                omitted_item_count: 0,
                categories: vec![ProposalContextManifestEntrySummary {
                    category: "assisted-ai".to_string(),
                    item_count: 1,
                    omitted_item_count: 0,
                    privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                    manifest_hash: Some(fingerprint("manifest:p5:context")),
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                }],
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            diff_summary: ProposalDiffSummary {
                kind: ProposalDiffSummaryKind::Text,
                target_count: 2,
                hunk_count: 1,
                inserted_line_count: 0,
                deleted_line_count: 0,
                omitted_hunk_count: 0,
                full_source_redacted: true,
                diff_hash: Some(fingerprint("ai-diff-summary")),
                chunks: Vec::new(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            preview_warnings: vec![ProposalPreviewWarning {
                code: "assisted_ai.preview.metadata_only".to_string(),
                kind: ProposalPreviewWarningKind::RawSourceRedacted,
                message: "Assisted AI preview omits raw source".to_string(),
                target_id: None,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            }],
            diagnostics: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        selected_proposal_id: Some(proposal_id),
        omitted_row_count: 0,
        generated_at: TimestampMillis(2100),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

#[test]
fn dto_contracts_assisted_ai_provider_capability_and_request_are_metadata_only() {
    let provider = assisted_ai_provider(AssistedAiProviderClass::LocalLoopback);
    let boundary = assisted_ai_boundary();
    let decision = assisted_ai_evaluate_route_decision(
        &provider,
        &boundary,
        AssistedAiOperationClass::ProposeEdit,
        1,
    );
    assert_eq!(
        decision.disposition,
        AssistedAiRequestDisposition::MetadataOnlyReady
    );
    assert_eq!(
        decision.provider_invocation,
        AssistedAiProviderInvocationState::NotEncoded
    );

    let request = AssistedAiRequestContract::new(
        "assist:req:1",
        provider,
        AssistedAiOperationClass::ProposeEdit,
        assisted_ai_ref(
            "manifest:p5:context",
            AssistedAiTrustProjectionKind::ContextManifest,
        ),
        assisted_ai_ref(
            "privacy:p5:inspector",
            AssistedAiTrustProjectionKind::PrivacyInspector,
        ),
        assisted_ai_ref(
            "budget:p5:projection",
            AssistedAiTrustProjectionKind::PermissionBudget,
        ),
        boundary
            .budget_evaluations
            .iter()
            .map(|evaluation| {
                AssistedAiPermissionBudgetEvaluationReference::from_evaluation(
                    evaluation,
                    fingerprint(&evaluation.evaluation_id),
                    1,
                )
            })
            .collect(),
        assisted_ai_ref(
            "checklist:p5:approval",
            AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
        ),
        Some(assisted_ai_ref(
            "rollback:p5:checkpoint",
            AssistedAiTrustProjectionKind::CheckpointRollback,
        )),
        CorrelationId(901),
        causality_id(),
        assisted_ai_proposal_intent(),
        decision,
        TimestampMillis(2000),
        1,
    )
    .expect("request contract should validate correlation metadata");

    let value = serde_json::to_value(&request).expect("serialize assisted AI request contract");
    assert_eq!(
        value["context_manifest"]["reference_id"],
        "manifest:p5:context"
    );
    assert_eq!(
        value["privacy_inspector"]["projection_hash"]["value"],
        "privacy:p5:inspector"
    );
    assert_eq!(
        value["permission_budget_evaluations"][0]["evaluation_id"],
        "eval:allowed"
    );
    assert_eq!(value["route_decision"]["provider_invocation"], "NotEncoded");
    assert_eq!(value["redaction_hints"], json!(["MetadataOnly"]));

    let serialized = serde_json::to_string(&value).expect("stringify assisted AI request");
    assert!(!serialized.contains("fn main"));
    assert!(!serialized.contains("raw prompt"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("provider_payload"));
    assert!(!serialized.contains("source_body"));
    assert!(!serialized.contains("terminal output"));
    assert!(!serialized.contains("ChatCompletionRequest"));
    assert!(!serialized.contains("runtime_started"));

    let roundtrip: AssistedAiRequestContract =
        serde_json::from_value(value.clone()).expect("deserialize assisted AI request");
    assert_eq!(
        roundtrip.context_manifest.reference_id,
        "manifest:p5:context"
    );
    assert_eq!(
        roundtrip.route_decision.provider_invocation,
        AssistedAiProviderInvocationState::NotEncoded
    );

    let mut missing = value;
    remove_required_field::<AssistedAiRequestContract>(&mut missing, "schema_version");
}

#[test]
fn dto_contracts_assisted_ai_gate_refusals_are_metadata_only_no_invocation() {
    let provider = assisted_ai_provider(AssistedAiProviderClass::HostedRemote);
    let cases = vec![
        (
            AssistedAiConsentBoundary {
                consent_state: AssistedAiConsentState::Missing,
                ..assisted_ai_boundary()
            },
            "consent.missing",
        ),
        (
            AssistedAiConsentBoundary {
                privacy_scope_allowed: false,
                ..assisted_ai_boundary()
            },
            "privacy.scope_denied",
        ),
        (
            AssistedAiConsentBoundary {
                budget_evaluations: vec![assisted_ai_budget_evaluation(
                    "eval:depleted",
                    PermissionBudgetState::Depleted,
                    PermissionBudgetEvaluationDisposition::RefusedDepleted,
                    false,
                )],
                ..assisted_ai_boundary()
            },
            "budget.depleted",
        ),
        (
            AssistedAiConsentBoundary {
                air_gap_mode: true,
                ..assisted_ai_boundary()
            },
            "egress.remote_denied",
        ),
        (
            AssistedAiConsentBoundary {
                workspace_trust_state: WorkspaceTrustState::Untrusted,
                ..assisted_ai_boundary()
            },
            "workspace.untrusted",
        ),
    ];

    for (boundary, reason_code) in cases {
        let decision = assisted_ai_evaluate_route_decision(
            &provider,
            &boundary,
            AssistedAiOperationClass::ProposeEdit,
            1,
        );
        assert_eq!(decision.disposition, AssistedAiRequestDisposition::Refused);
        assert_eq!(
            decision.provider_invocation,
            AssistedAiProviderInvocationState::NotEncoded
        );
        assert_eq!(
            decision.refusal.as_ref().expect("refusal").reason_code,
            reason_code
        );
        let serialized = serde_json::to_string(&decision).expect("serialize refusal decision");
        assert!(serialized.contains(reason_code));
        assert!(!serialized.contains("provider_payload"));
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("network_request"));
        assert!(!serialized.contains("tool_call"));
        assert!(!serialized.contains("agent_runtime"));
    }
}

#[test]
fn dto_contracts_assisted_ai_edit_output_converts_to_proposal_only_and_rejects_invalid_preconditions()
 {
    let output = AssistedAiEditProposalOutput {
        output_id: "assist:output:1".to_string(),
        request_id: "assist:req:1".to_string(),
        provider_id: "provider:local-redacted".to_string(),
        proposal_id: ProposalId(701),
        principal: PrincipalId("principal-1".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        payload: ProposalPayload::TextEdit(TextEditProposal {
            file_id: FileId(33),
            edits: EditBatch {
                edits: vec![TextEdit {
                    range: TextRange {
                        start: TextOffset {
                            value: 10,
                            encoding: TextCoordinateEncoding::Byte,
                        },
                        end: TextOffset {
                            value: 14,
                            encoding: TextCoordinateEncoding::Byte,
                        },
                    },
                    replacement: "renamed".to_string(),
                }],
            },
        }),
        preconditions: preconditions(),
        preview: PreviewSummary {
            summary: "assisted AI proposal preview".to_string(),
            details: vec!["bounded replacement text is present only inside edit DTOs".to_string()],
        },
        expires_at: Some(TimestampMillis(3000)),
        created_at: TimestampMillis(2000),
        context_manifest: assisted_ai_ref(
            "manifest:p5:context",
            AssistedAiTrustProjectionKind::ContextManifest,
        ),
        approval_checklist: assisted_ai_ref(
            "checklist:p5:approval",
            AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
        ),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let proposal = output
        .to_workspace_proposal()
        .expect("valid assisted AI output converts to proposal only");
    assert_eq!(proposal.proposal_id, ProposalId(701));
    assert_eq!(proposal.correlation_id, CorrelationId(901));
    assert_eq!(output.causality_id, causality_id());
    assert_eq!(proposal.preconditions.snapshot_id, Some(SnapshotId(66)));
    assert!(matches!(proposal.payload, ProposalPayload::TextEdit(_)));

    let serialized = serde_json::to_string(&output).expect("serialize assisted AI output");
    assert!(serialized.contains("replacement"));
    assert!(serialized.contains("renamed"));
    assert!(!serialized.contains("apply"));
    assert!(!serialized.contains("approved"));
    assert!(!serialized.contains("workspace_actor"));
    assert!(!serialized.contains("provider_payload"));
    assert!(!serialized.contains("raw prompt"));

    let missing_preconditions = AssistedAiEditProposalOutput {
        preconditions: empty_preconditions(),
        ..output.clone()
    };
    assert!(matches!(
        missing_preconditions.to_workspace_proposal(),
        Err(AssistedAiContractError::MissingPrecondition { .. })
    ));

    let missing_fingerprint = AssistedAiEditProposalOutput {
        preconditions: ProposalVersionPreconditions {
            expected_fingerprint: None,
            ..preconditions()
        },
        ..output.clone()
    };
    assert!(matches!(
        missing_fingerprint.to_workspace_proposal(),
        Err(AssistedAiContractError::MissingPrecondition { reason }) if reason == "missing.expected_fingerprint"
    ));

    let zero_correlation = AssistedAiEditProposalOutput {
        correlation_id: CorrelationId(0),
        ..output.clone()
    };
    assert!(matches!(
        zero_correlation.to_workspace_proposal(),
        Err(AssistedAiContractError::ZeroCorrelationId)
    ));

    let nil_causality = AssistedAiEditProposalOutput {
        causality_id: CausalityId(Uuid::nil()),
        ..output
    };
    assert!(matches!(
        nil_causality.to_workspace_proposal(),
        Err(AssistedAiContractError::NilCausalityId)
    ));
}

#[test]
fn dto_contracts_assisted_ai_projection_serializes_metadata_only_and_redacted() {
    let provider = assisted_ai_provider(AssistedAiProviderClass::LocalLoopback);
    let boundary = assisted_ai_boundary();
    let decision = assisted_ai_evaluate_route_decision(
        &provider,
        &boundary,
        AssistedAiOperationClass::ProposeEdit,
        1,
    );
    let request = assisted_ai_request_with_decision(provider.clone(), boundary, decision);
    let output = assisted_ai_output(ProposalId(701));
    let ledger = assisted_ai_ledger(ProposalId(701));
    let approval = ProposalApprovalChecklistProjection {
        checklist_id: "checklist:p5:approval".to_string(),
        proposal_id: ProposalId(701),
        workspace_id: Some(WorkspaceId(11)),
        payload_kind: ProposalPayloadKind::TextEdit,
        lifecycle_state: ProposalLifecycleState::Previewed,
        correlation_id: CorrelationId(901),
        causality_id: Some(causality_id()),
        ready_for_approval: true,
        gates: Vec::new(),
        blockers: Vec::new(),
        risk_labels: vec![ProposalRiskLabel::Low],
        privacy_labels: vec![ProposalPrivacyLabel::WorkspaceMetadata],
        explicit_denial_reasons: Vec::new(),
        generated_at: TimestampMillis(2100),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let projection = assisted_ai_projection_from_metadata(
        "assisted-ai:p6-2",
        vec![provider],
        vec![request],
        vec![output],
        Some(&ledger),
        None,
        None,
        None,
        Some(&approval),
        None,
        TimestampMillis(2200),
        1,
    );

    assert_eq!(projection.provider_count, 1);
    assert_eq!(projection.request_count, 1);
    assert_eq!(projection.preview_ready_count, 1);
    assert_eq!(
        projection.provider_invocation,
        AssistedAiProviderInvocationState::NotEncoded
    );
    assert!(projection.proposal_previews[0].ready_for_preview);
    assert!(projection.proposal_previews[0].ready_for_approval);
    assert!(!projection.proposal_previews[0].ready_for_apply);
    assert_eq!(
        projection.proposal_previews[0].readiness,
        AssistedAiProposalPreviewReadiness::PreviewReady
    );
    assert_eq!(
        projection.proposal_previews[0].preconditions.snapshot_id,
        Some(SnapshotId(66))
    );
    assert_eq!(projection.proposal_previews[0].diff_summary.hunk_count, 1);
    assert!(projection.proposal_previews[0].ledger_row_present);

    let value = serde_json::to_value(&projection).expect("serialize assisted AI projection");
    assert_eq!(value["proposal_previews"][0]["ready_for_apply"], false);
    assert_eq!(value["routes"][0]["provider_invocation"], "NotEncoded");
    assert_eq!(value["proposal_previews"][0]["proposal_id"], 701);

    let serialized = serde_json::to_string(&value).expect("stringify assisted AI projection");
    assert!(serialized.contains("PreviewReady"));
    assert!(serialized.contains("proposal.apply.not_encoded"));
    assert!(!serialized.contains("renamed"));
    assert!(!serialized.contains("replacement"));
    assert!(!serialized.contains("fn main"));
    assert!(!serialized.contains("raw prompt"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("provider_payload"));
    assert!(!serialized.contains("terminal output"));
    assert!(!serialized.contains("network_request"));
    assert!(!serialized.contains("tool_call"));
    assert!(!serialized.contains("agent_runtime"));
    assert!(!serialized.contains("runtime_started"));

    let roundtrip: AssistedAiProjection =
        serde_json::from_value(value.clone()).expect("deserialize assisted AI projection");
    assert_eq!(roundtrip.proposal_previews[0].proposal_id, ProposalId(701));
    assert_eq!(
        roundtrip.provider_invocation,
        AssistedAiProviderInvocationState::NotEncoded
    );

    let mut missing = value;
    remove_required_field::<AssistedAiProjection>(&mut missing, "schema_version");
}

#[test]
fn dto_contracts_assisted_ai_refused_route_does_not_produce_preview_ready_state() {
    let provider = assisted_ai_provider(AssistedAiProviderClass::HostedRemote);
    let boundary = AssistedAiConsentBoundary {
        consent_state: AssistedAiConsentState::Missing,
        ..assisted_ai_boundary()
    };
    let decision = assisted_ai_evaluate_route_decision(
        &provider,
        &boundary,
        AssistedAiOperationClass::ProposeEdit,
        1,
    );
    let request = assisted_ai_request_with_decision(provider.clone(), boundary, decision);
    let output = assisted_ai_output(ProposalId(702));
    let ledger = assisted_ai_ledger(ProposalId(702));

    let projection = assisted_ai_projection_from_metadata(
        "assisted-ai:refused",
        vec![provider],
        vec![request],
        vec![output],
        Some(&ledger),
        None,
        None,
        None,
        None,
        None,
        TimestampMillis(2200),
        1,
    );

    assert_eq!(projection.preview_ready_count, 0);
    assert_eq!(projection.refusal_count, 2);
    let preview = &projection.proposal_previews[0];
    assert_eq!(
        preview.readiness,
        AssistedAiProposalPreviewReadiness::RouteRefused
    );
    assert!(!preview.ready_for_preview);
    assert!(!preview.ready_for_approval);
    assert!(!preview.ready_for_apply);
    assert_eq!(
        preview.refusal.as_ref().unwrap().reason_code,
        "consent.missing"
    );
    assert_eq!(
        projection.routes[0].provider_invocation,
        AssistedAiProviderInvocationState::NotEncoded
    );

    let serialized = serde_json::to_string(&projection).expect("serialize refused projection");
    assert!(serialized.contains("RouteRefused"));
    assert!(serialized.contains("consent.missing"));
    assert!(!serialized.contains("ChatCompletionRequest"));
    assert!(!serialized.contains("provider_payload"));
    assert!(!serialized.contains("raw prompt"));
    assert!(!serialized.contains("network_request"));
    assert!(!serialized.contains("tool_call"));
    assert!(!serialized.contains("agent_runtime"));
}

#[test]
fn dto_contracts_assisted_ai_audit_record_is_metadata_only_and_validated() {
    let record = assisted_ai_audit(Some(ProposalId(701)));
    record.validate().expect("metadata-only audit validates");

    let value = serde_json::to_value(&record).expect("serialize assisted AI audit record");
    assert_eq!(value["proposal_id"], 701);
    assert_eq!(value["runtime_invocation_state"], "NotEncoded");
    assert_eq!(value["redaction_hints"], json!(["MetadataOnly"]));
    assert_eq!(value["budget_dispositions"][0], "Allowed");
    assert_eq!(value["preview_id"], "assist:preview:701");

    let serialized = serde_json::to_string(&value).expect("stringify assisted AI audit");
    assert!(serialized.contains("ProposalPreviewReady"));
    assert!(serialized.contains("provider.invocation.not_encoded"));
    assert!(!serialized.contains("fn main"));
    assert!(!serialized.contains("raw prompt"));
    assert!(!serialized.contains("source_body"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("provider_payload"));
    assert!(!serialized.contains("terminal output"));
    assert!(!serialized.contains("ChatCompletionRequest"));
    assert!(!serialized.contains("network_request"));
    assert!(!serialized.contains("tool_call"));
    assert!(!serialized.contains("runtime_started"));

    let roundtrip: AssistedAiAuditRecord =
        serde_json::from_value(value.clone()).expect("deserialize assisted AI audit");
    assert_eq!(roundtrip.proposal_id, Some(ProposalId(701)));
    assert_eq!(roundtrip.event_sequence, EventSequence(77));

    let mut zero_correlation = record.clone();
    zero_correlation.correlation_id = CorrelationId(0);
    assert!(matches!(
        zero_correlation.validate(),
        Err(AssistedAiContractError::ZeroCorrelationId)
    ));

    let mut nil_causality = record.clone();
    nil_causality.causality_id = CausalityId(Uuid::nil());
    assert!(matches!(
        nil_causality.validate(),
        Err(AssistedAiContractError::NilCausalityId)
    ));

    let mut zero_sequence = record.clone();
    zero_sequence.event_sequence = EventSequence(0);
    assert!(matches!(
        zero_sequence.validate(),
        Err(AssistedAiContractError::ZeroEventSequence)
    ));

    let mut raw_marker = record;
    raw_marker.refusal_error_category = Some("raw prompt provider_payload".to_string());
    assert!(matches!(
        raw_marker.validate(),
        Err(AssistedAiContractError::NonMetadataOnlyAuditRecord { .. })
    ));
}

#[test]
fn dto_contracts_phase4_runtime_audit_allows_runtime_states_but_rejects_raw_markers() {
    let mut record = Phase4RuntimeAuditRecord {
        audit_id: "phase4:audit:1".to_string(),
        run_id: Some(AgentRunId("run-1".to_string())),
        step_id: Some(AgentStepId("step-1".to_string())),
        provider_route_id: Some("route-1".to_string()),
        invocation_state: AssistedAiProviderInvocationState::Completed,
        outcome_label: "provider.completed.metadata_only".to_string(),
        labels: vec!["local.provider".to_string()],
        correlation_id: CorrelationId(42),
        causality_id: causality_id(),
        event_sequence: EventSequence(7),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    validate_phase4_runtime_audit_record(&record).expect("runtime audit metadata is valid");

    record.labels.push("raw prompt: secret".to_string());
    assert!(matches!(
        validate_phase4_runtime_audit_record(&record),
        Err(AssistedAiContractError::NonMetadataOnlyAuditRecord { .. })
    ));
}

#[test]
fn dto_contracts_provider_route_request_requires_complete_runtime_metadata() {
    let mut request = AssistedAiProviderRouteRequest {
        route_id: "route-1".to_string(),
        provider_id: "deterministic-local".to_string(),
        model_label: "local-test".to_string(),
        provider_class: AssistedAiProviderClass::LocalLoopback,
        operation_class: AssistedAiOperationClass::ProposeEdit,
        context_manifest: trust_reference("ctx-1", AssistedAiTrustProjectionKind::ContextManifest),
        privacy_inspector: trust_reference(
            "privacy-1",
            AssistedAiTrustProjectionKind::PrivacyInspector,
        ),
        permission_budget: trust_reference(
            "budget-1",
            AssistedAiTrustProjectionKind::PermissionBudget,
        ),
        proposal_intent: AssistedAiProposalTargetIntent {
            payload_kind: ProposalPayloadKind::TextEdit,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: Vec::new(),
                omitted_target_count: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            required_capability: CapabilityId("ai.proposal.create".to_string()),
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            labels: vec!["proposal.intent".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        policy_decision_id: Some(CapabilityDecisionId(9)),
        required_capability: CapabilityId("ai.provider.invoke".to_string()),
        network_target: Some(NetworkTarget {
            scheme: "http".to_string(),
            host: "localhost".to_string(),
            port: Some(11434),
        }),
        cancellation_token: cancellation_token_id(),
        health_labels: vec!["healthy".to_string()],
        cost_labels: vec!["local".to_string()],
        principal_id: PrincipalId("principal-1".to_string()),
        workspace_trust_state: WorkspaceTrustState::Trusted,
        correlation_id: CorrelationId(42),
        causality_id: causality_id(),
        event_sequence: EventSequence(8),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    validate_assisted_ai_provider_route_request(&request).expect("route request is valid");

    request.event_sequence = EventSequence(0);
    assert_eq!(
        validate_assisted_ai_provider_route_request(&request),
        Err(AssistedAiContractError::ZeroEventSequence)
    );

    request.event_sequence = EventSequence(8);
    request.cancellation_token = CancellationTokenId(Uuid::nil());
    assert!(matches!(
        validate_assisted_ai_provider_route_request(&request),
        Err(AssistedAiContractError::NonMetadataOnlyAuditRecord { .. })
    ));
}

#[test]
fn dto_contracts_assisted_ai_audit_outcomes_are_metadata_only_without_invocation() {
    let mut route_refused = assisted_ai_audit(None);
    route_refused.outcome_category = AssistedAiAuditOutcomeCategory::RouteRefused;
    route_refused.refusal_error_category = Some("consent.missing".to_string());
    route_refused.preview_id = None;
    route_refused.preview_hash = None;

    let mut consent_denied = assisted_ai_audit(None);
    consent_denied.outcome_category = AssistedAiAuditOutcomeCategory::ConsentDenied;
    consent_denied.consent_disposition = Some(AssistedAiConsentState::Denied);
    consent_denied.refusal_error_category = Some("consent.denied".to_string());

    let mut privacy_denied = assisted_ai_audit(None);
    privacy_denied.outcome_category = AssistedAiAuditOutcomeCategory::PrivacyDenied;
    privacy_denied.privacy_disposition = AssistedAiAuditPrivacyDisposition::Denied;
    privacy_denied.refusal_error_category = Some("privacy.denied".to_string());

    let mut budget_denied = assisted_ai_audit(None);
    budget_denied.outcome_category = AssistedAiAuditOutcomeCategory::BudgetDenied;
    budget_denied.budget_dispositions = vec![PermissionBudgetEvaluationDisposition::RefusedDenied];
    budget_denied.refusal_error_category = Some("budget.denied".to_string());

    let mut invalid_preconditions = assisted_ai_audit(None);
    invalid_preconditions.outcome_category = AssistedAiAuditOutcomeCategory::InvalidPreconditions;
    invalid_preconditions.refusal_error_category = Some("missing.expected_fingerprint".to_string());

    let preview_ready = assisted_ai_audit(Some(ProposalId(701)));

    for record in [
        route_refused,
        consent_denied,
        privacy_denied,
        budget_denied,
        invalid_preconditions,
        preview_ready,
    ] {
        record.validate().expect("auditable metadata-only outcome");
        let serialized =
            serde_json::to_string(&record).expect("serialize assisted AI audit outcome");
        assert!(serialized.contains("NotEncoded"));
        assert!(serialized.contains("MetadataOnly"));
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("source_body"));
        assert!(!serialized.contains("provider_payload"));
        assert!(!serialized.contains("ChatCompletionRequest"));
        assert!(!serialized.contains("terminal output"));
        assert!(!serialized.contains("full diff"));
        assert!(!serialized.contains("runtime_started"));
        assert!(!serialized.contains("approved"));
        assert!(!serialized.contains("applied"));
    }
}

#[test]
fn dto_contracts_assisted_ai_invalid_output_does_not_produce_preview_ready_state() {
    let provider = assisted_ai_provider(AssistedAiProviderClass::Local);
    let boundary = assisted_ai_boundary();
    let decision = assisted_ai_evaluate_route_decision(
        &provider,
        &boundary,
        AssistedAiOperationClass::ProposeEdit,
        1,
    );
    let request = assisted_ai_request_with_decision(provider.clone(), boundary, decision);
    let output = AssistedAiEditProposalOutput {
        preconditions: empty_preconditions(),
        ..assisted_ai_output(ProposalId(703))
    };
    let ledger = assisted_ai_ledger(ProposalId(703));

    let projection = assisted_ai_projection_from_metadata(
        "assisted-ai:invalid-output",
        vec![provider],
        vec![request],
        vec![output],
        Some(&ledger),
        None,
        None,
        None,
        None,
        None,
        TimestampMillis(2200),
        1,
    );

    let preview = &projection.proposal_previews[0];
    assert_eq!(
        preview.readiness,
        AssistedAiProposalPreviewReadiness::InvalidOutput
    );
    assert!(!preview.ready_for_preview);
    assert!(!preview.ready_for_approval);
    assert!(!preview.ready_for_apply);
    assert_eq!(
        preview.refusal.as_ref().unwrap().reason_code,
        "missing.file_content_version"
    );
    assert_eq!(
        projection.provider_invocation,
        AssistedAiProviderInvocationState::NotEncoded
    );
}

fn delegated_task_ref(
    reference_id: &str,
    kind: AssistedAiTrustProjectionKind,
) -> AssistedAiTrustProjectionReference {
    AssistedAiTrustProjectionReference {
        reference_id: reference_id.to_string(),
        kind,
        projection_hash: fingerprint(reference_id),
        schema_version: 1,
    }
}

fn delegated_task_target() -> DelegatedTaskAffectedTargetSummary {
    DelegatedTaskAffectedTargetSummary {
        target_id: "target-buffer-main".to_string(),
        kind: ProposalTargetKind::OpenBuffer,
        workspace_id: Some(WorkspaceId(11)),
        file_id: Some(FileId(33)),
        buffer_id: Some(BufferId(22)),
        ranges: vec![ByteRange::new(10, 14)],
        hashes: vec![fingerprint("target-summary")],
        counts: vec![ContextManifestItemCount {
            label: "ranges".to_string(),
            count: 1,
        }],
        labels: vec!["bounded-target-name".to_string()],
        risk_label: ProposalRiskLabel::Medium,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn delegated_task_preview_link(proposal_id: ProposalId) -> DelegatedTaskProposalPreviewLink {
    DelegatedTaskProposalPreviewLink {
        link_id: format!("delegated-task:proposal-preview:{}", proposal_id.0),
        proposal_id,
        payload_kind: ProposalPayloadKind::TextEdit,
        lifecycle_state: ProposalLifecycleState::Previewed,
        approval_checklist: Some(delegated_task_ref(
            "checklist:p5:approval",
            AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
        )),
        checkpoint_rollback: Some(delegated_task_ref(
            "rollback:p5:checkpoint",
            AssistedAiTrustProjectionKind::CheckpointRollback,
        )),
        target_count: 1,
        hunk_count: 1,
        full_source_redacted: true,
        risk_label: ProposalRiskLabel::Medium,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn delegated_task_step(proposal_id: ProposalId) -> DelegatedTaskPlanStep {
    DelegatedTaskPlanStep {
        step_id: DelegatedTaskStepId("step:proposal-preview".to_string()),
        order: 1,
        objective_summary_hash: fingerprint("step-objective-summary"),
        operation_class: DelegatedTaskOperationClass::LinkProposalPreview,
        depends_on: Vec::new(),
        required_gates: vec![
            DelegatedTaskTrustGateKind::ContextManifest,
            DelegatedTaskTrustGateKind::ApprovalChecklist,
            DelegatedTaskTrustGateKind::Rollback,
        ],
        target_ids: vec!["target-buffer-main".to_string()],
        proposal_preview: Some(delegated_task_preview_link(proposal_id)),
        state: DelegatedTaskStepState::ProposalPreviewLinked,
        blockers: Vec::new(),
        labels: vec!["proposal-preview-link-only".to_string()],
        counts: vec![ContextManifestItemCount {
            label: "proposal_previews".to_string(),
            count: 1,
        }],
        risk_label: ProposalRiskLabel::Medium,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn delegated_task_input(proposal_id: ProposalId) -> DelegatedTaskPlanningBoundaryInput {
    DelegatedTaskPlanningBoundaryInput {
        plan_id: DelegatedTaskPlanId("plan:p7-1:metadata".to_string()),
        workspace_id: Some(WorkspaceId(11)),
        objective_summary_hash: fingerprint("objective-summary"),
        allowed_operation_classes: vec![
            DelegatedTaskOperationClass::ReadContextMetadata,
            DelegatedTaskOperationClass::ReferenceAssistedAiMetadata,
            DelegatedTaskOperationClass::DraftProposalMetadata,
            DelegatedTaskOperationClass::LinkProposalPreview,
            DelegatedTaskOperationClass::RequestHumanApproval,
            DelegatedTaskOperationClass::CheckRollbackReadiness,
        ],
        context_manifest: Some(delegated_task_ref(
            "manifest:p5:context",
            AssistedAiTrustProjectionKind::ContextManifest,
        )),
        privacy_inspector: Some(delegated_task_ref(
            "privacy:p5:inspector",
            AssistedAiTrustProjectionKind::PrivacyInspector,
        )),
        permission_budget_projection: Some(delegated_task_ref(
            "budget:p5:projection",
            AssistedAiTrustProjectionKind::PermissionBudget,
        )),
        approval_checklist: Some(delegated_task_ref(
            "checklist:p5:approval",
            AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
        )),
        checkpoint_rollback: Some(delegated_task_ref(
            "rollback:p5:checkpoint",
            AssistedAiTrustProjectionKind::CheckpointRollback,
        )),
        assisted_ai_projection: Some(delegated_task_ref(
            "assisted-ai:p6-2",
            AssistedAiTrustProjectionKind::AssistedAiProjection,
        )),
        assisted_ai_required: true,
        affected_targets: vec![delegated_task_target()],
        steps: vec![delegated_task_step(proposal_id)],
        proposal_preview_links: vec![delegated_task_preview_link(proposal_id)],
        workspace_trust_state: WorkspaceTrustState::Trusted,
        privacy_denied: false,
        permission_budget_denied: false,
        permission_budget_depleted: false,
        approval_checklist_valid: true,
        checkpoint_required: true,
        checkpoint_available: true,
        rollback_required: true,
        rollback_available: true,
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        created_at: TimestampMillis(2300),
        schema_version: 1,
    }
}

fn delegated_task_audit_ref(
    proposal_id: Option<ProposalId>,
) -> DelegatedTaskAssistedAiAuditReference {
    DelegatedTaskAssistedAiAuditReference {
        audit_id: "assist:audit:req-1:77".to_string(),
        audit_hash: fingerprint("assist-audit-hash"),
        request_contract_id: "assist:req:1".to_string(),
        request_contract_hash: fingerprint("assist-request-hash"),
        projection_id: Some("assisted-ai:p6-3".to_string()),
        projection_hash: Some(fingerprint("assisted-ai-projection-hash")),
        preview_id: Some("assist:preview:801".to_string()),
        preview_hash: Some(fingerprint("assist-preview-hash")),
        proposal_id,
        outcome_category: AssistedAiAuditOutcomeCategory::ProposalPreviewReady,
        event_sequence: EventSequence(77),
        redaction_state: AssistedAiAuditRedactionState::MetadataOnly,
        runtime_invocation_state: AssistedAiProviderInvocationState::NotEncoded,
        schema_version: 1,
    }
}

#[test]
fn dto_contracts_delegated_task_plan_and_projection_are_metadata_only() {
    let plan = delegated_task_plan_from_boundary_input(delegated_task_input(ProposalId(801)));
    assert_eq!(plan.plan_state, DelegatedTaskPlanState::AwaitingApproval);
    assert_eq!(
        plan.audit_readiness.runtime_activation,
        DelegatedTaskRuntimeActivationState::NotEncoded
    );
    assert!(plan.blockers.is_empty());
    assert!(plan.refusals.is_empty());
    assert_eq!(plan.proposal_preview_links[0].proposal_id, ProposalId(801));

    let projection = delegated_task_projection_from_plan_contracts(
        "delegated-task:p7-1",
        vec![plan.clone()],
        TimestampMillis(2400),
        1,
    );
    assert_eq!(projection.plan_count, 1);
    assert_eq!(projection.blocked_plan_count, 0);
    assert_eq!(projection.refused_plan_count, 0);
    assert_eq!(
        projection.runtime_activation,
        DelegatedTaskRuntimeActivationState::NotEncoded
    );
    assert_eq!(projection.plan_rows[0].proposal_preview_link_count, 1);
    assert_eq!(
        projection.step_summaries[0].proposal_id,
        Some(ProposalId(801))
    );

    let value = serde_json::to_value(&projection).expect("serialize delegated task projection");
    assert_eq!(value["runtime_activation"], "NotEncoded");
    assert_eq!(value["proposal_preview_links"][0]["proposal_id"], 801);
    let serialized = serde_json::to_string(&value).expect("stringify delegated task projection");
    assert!(serialized.contains("delegated_task.plan_only_no_runtime"));
    assert!(serialized.contains("outputs.must_be_proposals_only"));
    assert!(!serialized.contains("fn main"));
    assert!(!serialized.contains("raw prompt"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("provider_payload"));
    assert!(!serialized.contains("terminal output"));
    assert!(!serialized.contains("replacement"));
    assert!(!serialized.contains("network_request"));
    assert!(!serialized.contains("tool_call"));
    assert!(!serialized.contains("agent_runtime"));
    assert!(!serialized.contains("runtime_started"));

    let roundtrip: DelegatedTaskProjection =
        serde_json::from_value(value.clone()).expect("deserialize delegated task projection");
    assert_eq!(roundtrip.plan_rows[0].plan_id, plan.plan_id);
    let mut missing = value;
    remove_required_field::<DelegatedTaskProjection>(&mut missing, "schema_version");
}

#[test]
fn dto_contracts_delegated_task_blockers_and_refusals_are_visible_without_runtime() {
    let blocked = delegated_task_plan_from_boundary_input(DelegatedTaskPlanningBoundaryInput {
        privacy_denied: true,
        permission_budget_depleted: true,
        approval_checklist: None,
        approval_checklist_valid: false,
        checkpoint_available: false,
        rollback_available: false,
        correlation_id: CorrelationId(0),
        causality_id: CausalityId(Uuid::nil()),
        ..delegated_task_input(ProposalId(802))
    });

    assert_eq!(blocked.plan_state, DelegatedTaskPlanState::Refused);
    assert_eq!(
        blocked.audit_readiness.readiness,
        DelegatedTaskPlanReadinessStatus::Refused
    );
    assert!(!blocked.blockers.is_empty());
    assert!(!blocked.refusals.is_empty());

    let blocker_codes = blocked
        .blockers
        .iter()
        .map(|blocker| blocker.reason_code.as_str())
        .collect::<Vec<_>>();
    assert!(blocker_codes.contains(&"approval_checklist.missing"));
    assert!(blocker_codes.contains(&"checkpoint.missing"));
    assert!(blocker_codes.contains(&"rollback.missing"));

    let refusal_codes = blocked
        .refusals
        .iter()
        .map(|refusal| refusal.reason_code.as_str())
        .collect::<Vec<_>>();
    assert!(refusal_codes.contains(&"privacy.denied"));
    assert!(refusal_codes.contains(&"budget.depleted"));
    assert!(refusal_codes.contains(&"correlation.zero"));
    assert!(refusal_codes.contains(&"causality.nil"));

    let projection = delegated_task_projection_from_plan_contracts(
        "delegated-task:blockers",
        vec![blocked],
        TimestampMillis(2400),
        1,
    );
    assert_eq!(projection.refused_plan_count, 1);
    assert_eq!(projection.blockers.len(), 3);
    assert_eq!(projection.refusals.len(), 4);

    let serialized = serde_json::to_string(&projection).expect("serialize blocked delegated task");
    assert!(serialized.contains("approval_checklist.missing"));
    assert!(serialized.contains("checkpoint.missing"));
    assert!(serialized.contains("rollback.missing"));
    assert!(serialized.contains("privacy.denied"));
    assert!(serialized.contains("budget.depleted"));
    assert!(serialized.contains("correlation.zero"));
    assert!(serialized.contains("causality.nil"));
    assert!(serialized.contains("NotEncoded"));
    assert!(!serialized.contains("ProviderRequest"));
    assert!(!serialized.contains("ChatCompletionRequest"));
    assert!(!serialized.contains("std::process"));
    assert!(!serialized.contains("thread::spawn"));
    assert!(!serialized.contains("tool_call"));
    assert!(!serialized.contains("terminal output"));
}

#[test]
fn dto_contracts_delegated_task_steps_link_proposals_and_trust_without_mutation() {
    let mut input = delegated_task_input(ProposalId(803));
    input.allowed_operation_classes = vec![
        DelegatedTaskOperationClass::ReadContextMetadata,
        DelegatedTaskOperationClass::LinkProposalPreview,
        DelegatedTaskOperationClass::CheckCheckpointMetadata,
    ];
    let plan = delegated_task_plan_from_boundary_input(input);
    let projection = delegated_task_projection_from_plan_contracts(
        "delegated-task:proposal-link",
        vec![plan],
        TimestampMillis(2400),
        1,
    );

    assert_eq!(
        projection.proposal_preview_links[0].proposal_id,
        ProposalId(803)
    );
    assert_eq!(
        projection.proposal_preview_links[0].payload_kind,
        ProposalPayloadKind::TextEdit
    );
    assert_eq!(projection.required_approvals.len(), 9);
    assert_eq!(
        projection.step_summaries[0].operation_class,
        DelegatedTaskOperationClass::LinkProposalPreview
    );
    assert_eq!(
        projection.step_summaries[0].state,
        DelegatedTaskStepState::ProposalPreviewLinked
    );

    let serialized = serde_json::to_string(&projection).expect("serialize proposal-linked plan");
    assert!(serialized.contains("manifest:p5:context"));
    assert!(serialized.contains("privacy:p5:inspector"));
    assert!(serialized.contains("budget:p5:projection"));
    assert!(serialized.contains("checklist:p5:approval"));
    assert!(serialized.contains("rollback:p5:checkpoint"));
    assert!(serialized.contains("assisted-ai:p6-2"));
    assert!(serialized.contains("proposal-preview-link-only"));
    assert!(!serialized.contains("ApplyProposal"));
    assert!(!serialized.contains("workspace_actor"));
    assert!(!serialized.contains("provider_payload"));
    assert!(!serialized.contains("network_request"));
    assert!(!serialized.contains("tool_call"));
    assert!(!serialized.contains("agent_runtime"));
    assert!(!serialized.contains("runtime_started"));
}

#[test]
fn dto_contracts_delegated_task_readiness_audit_linkage_is_metadata_only_and_validated() {
    let plan = delegated_task_plan_from_boundary_input(delegated_task_input(ProposalId(804)));
    let audit_ref = delegated_task_audit_ref(Some(ProposalId(804)));
    let record = delegated_task_audit_linkage_record(
        "delegated-task:audit-linkage:plan:p7-2",
        &plan,
        fingerprint("delegated-plan-metadata"),
        vec![audit_ref.clone()],
        EventSequence(88),
        1,
    )
    .expect("delegated task linkage should validate");

    assert_eq!(record.plan_id, plan.plan_id);
    assert_eq!(record.step_ids.len(), 1);
    assert_eq!(record.proposal_ids, vec![ProposalId(804)]);
    assert_eq!(record.assisted_ai_audit_references[0], audit_ref);
    assert_eq!(
        record.runtime_activation,
        DelegatedTaskRuntimeActivationState::NotEncoded
    );
    assert_eq!(
        record.readiness_classification,
        DelegatedTaskReadinessClassification::WaitingForApproval
    );
    record.validate().expect("linkage validates");

    let value = serde_json::to_value(&record).expect("serialize delegated task linkage");
    assert_eq!(value["event_sequence"], 88);
    assert_eq!(value["runtime_activation"], "NotEncoded");
    assert_eq!(value["readiness_classification"], "WaitingForApproval");

    let serialized = serde_json::to_string(&value).expect("stringify delegated task linkage");
    assert!(serialized.contains("assist:audit:req-1:77"));
    assert!(serialized.contains("assist-audit-hash"));
    assert!(serialized.contains("proposal.apply.not_encoded"));
    assert!(!serialized.contains("fn main"));
    assert!(!serialized.contains("raw prompt"));
    assert!(!serialized.contains("source_body"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("provider_payload"));
    assert!(!serialized.contains("terminal output"));
    assert!(!serialized.contains("ChatCompletionRequest"));
    assert!(!serialized.contains("network_request"));
    assert!(!serialized.contains("tool_call"));
    assert!(!serialized.contains("agent_runtime"));
    assert!(!serialized.contains("runtime_started"));
    assert!(!serialized.contains("replacement"));
    assert!(!serialized.contains("approved"));
    assert!(!serialized.contains("applied"));

    let roundtrip: DelegatedTaskAuditLinkageRecord =
        serde_json::from_value(value.clone()).expect("deserialize delegated task linkage");
    assert_eq!(roundtrip.proposal_ids, vec![ProposalId(804)]);
    let mut missing = value;
    remove_required_field::<DelegatedTaskAuditLinkageRecord>(&mut missing, "schema_version");
}

#[test]
fn dto_contracts_delegated_task_readiness_rejects_invalid_core_ids_and_raw_markers() {
    let plan = delegated_task_plan_from_boundary_input(delegated_task_input(ProposalId(805)));
    let record = delegated_task_audit_linkage_record(
        "delegated-task:audit-linkage:invalid-check",
        &plan,
        fingerprint("delegated-plan-metadata"),
        vec![delegated_task_audit_ref(Some(ProposalId(805)))],
        EventSequence(89),
        1,
    )
    .expect("valid linkage");

    let mut zero_correlation = record.clone();
    zero_correlation.correlation_id = CorrelationId(0);
    assert!(matches!(
        zero_correlation.validate(),
        Err(AssistedAiContractError::ZeroCorrelationId)
    ));

    let mut nil_causality = record.clone();
    nil_causality.causality_id = CausalityId(Uuid::nil());
    assert!(matches!(
        nil_causality.validate(),
        Err(AssistedAiContractError::NilCausalityId)
    ));

    let mut zero_sequence = record.clone();
    zero_sequence.event_sequence = EventSequence(0);
    assert!(matches!(
        zero_sequence.validate(),
        Err(AssistedAiContractError::ZeroEventSequence)
    ));

    let mut raw_marker = record.clone();
    raw_marker
        .runtime_activation_labels
        .push("agent_runtime runtime_started".to_string());
    assert!(matches!(
        raw_marker.validate(),
        Err(AssistedAiContractError::NonMetadataOnlyAuditRecord { .. })
    ));
}

#[test]
fn dto_contracts_delegated_task_readiness_classifies_waiting_blocked_refused_and_invalid_states() {
    let plan = delegated_task_plan_from_boundary_input(delegated_task_input(ProposalId(806)));
    assert_eq!(
        classify_delegated_task_readiness(&plan, &[]),
        DelegatedTaskReadinessClassification::WaitingForAudit
    );
    assert_eq!(
        classify_delegated_task_readiness(
            &plan,
            &[delegated_task_audit_ref(Some(ProposalId(806)))]
        ),
        DelegatedTaskReadinessClassification::WaitingForApproval
    );

    let waiting_preview =
        delegated_task_plan_from_boundary_input(DelegatedTaskPlanningBoundaryInput {
            proposal_preview_links: Vec::new(),
            assisted_ai_required: false,
            assisted_ai_projection: None,
            steps: vec![DelegatedTaskPlanStep {
                proposal_preview: None,
                ..delegated_task_step(ProposalId(807))
            }],
            ..delegated_task_input(ProposalId(807))
        });
    assert_eq!(
        classify_delegated_task_readiness(&waiting_preview, &[]),
        DelegatedTaskReadinessClassification::WaitingForProposalPreview
    );

    let blocked = delegated_task_plan_from_boundary_input(DelegatedTaskPlanningBoundaryInput {
        approval_checklist: None,
        approval_checklist_valid: false,
        ..delegated_task_input(ProposalId(808))
    });
    assert_eq!(
        classify_delegated_task_readiness(&blocked, &[]),
        DelegatedTaskReadinessClassification::Blocked
    );
    assert!(
        blocked
            .blockers
            .iter()
            .any(|blocker| blocker.reason_code == "approval_checklist.missing")
    );

    let refused = delegated_task_plan_from_boundary_input(DelegatedTaskPlanningBoundaryInput {
        privacy_denied: true,
        permission_budget_denied: true,
        ..delegated_task_input(ProposalId(809))
    });
    assert_eq!(
        classify_delegated_task_readiness(&refused, &[]),
        DelegatedTaskReadinessClassification::Refused
    );
    assert!(
        refused
            .refusals
            .iter()
            .any(|refusal| refusal.reason_code == "privacy.denied")
    );
    assert!(
        refused
            .refusals
            .iter()
            .any(|refusal| refusal.reason_code == "budget.denied")
    );

    let invalid = delegated_task_plan_from_boundary_input(DelegatedTaskPlanningBoundaryInput {
        correlation_id: CorrelationId(0),
        ..delegated_task_input(ProposalId(810))
    });
    assert_eq!(
        classify_delegated_task_readiness(&invalid, &[]),
        DelegatedTaskReadinessClassification::InvalidMetadata
    );
}

fn future_surface_input(
    surface_class: FutureSurfaceClass,
    requested_operation_classes: Vec<FutureSurfaceOperationClass>,
) -> FutureSurfacePlanningGateInput {
    FutureSurfacePlanningGateInput {
        gate_id: FutureSurfaceGateId(format!("future-surface:{surface_class:?}:p8-1")),
        surface_class,
        allowed_operation_classes: vec![
            FutureSurfaceOperationClass::MetadataProjection,
            FutureSurfaceOperationClass::ProposalOnlyEditOutput,
            FutureSurfaceOperationClass::TerminalCommandProposal,
            FutureSurfaceOperationClass::PluginIntentProposal,
            FutureSurfaceOperationClass::CollaborationEditProposal,
            FutureSurfaceOperationClass::RemoteWorkspaceProposal,
            FutureSurfaceOperationClass::AutonomousPlanProposal,
        ],
        denied_operation_classes: vec![
            FutureSurfaceOperationClass::EditorMutation,
            FutureSurfaceOperationClass::WorkspaceOrStorageMutation,
        ],
        requested_operation_classes,
        adr_status: FutureSurfaceRequirementStatus::Accepted,
        dependency_policy_status: FutureSurfaceRequirementStatus::Accepted,
        contract_test_status: FutureSurfaceRequirementStatus::Accepted,
        threat_model_status: FutureSurfaceRequirementStatus::Accepted,
        phase_status_entry_status: FutureSurfaceRequirementStatus::Accepted,
        workspace_trust_state: WorkspaceTrustState::Trusted,
        privacy_denied: false,
        permission_budget_denied: false,
        permission_budget_depleted: false,
        approval_required: true,
        approval_available: true,
        checkpoint_required: true,
        checkpoint_available: true,
        rollback_required: true,
        rollback_available: true,
        proposal_only_mutation_required: true,
        proposal_only_mutation_available: true,
        runtime_activation: FutureSurfaceRuntimeActivationState::NotEncoded,
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        labels: vec!["future_surface.metadata_only".to_string()],
        risk_labels: vec![ProposalRiskLabel::Medium],
        privacy_labels: vec![ProposalPrivacyLabel::WorkspaceMetadata],
        generated_at: TimestampMillis(2600),
        schema_version: 1,
    }
}

#[test]
fn dto_contracts_future_surface_gate_projection_serializes_metadata_only_and_redacted() {
    let gates = [
        (
            FutureSurfaceClass::Terminal,
            FutureSurfaceOperationClass::TerminalCommandProposal,
        ),
        (
            FutureSurfaceClass::Plugin,
            FutureSurfaceOperationClass::PluginIntentProposal,
        ),
        (
            FutureSurfaceClass::Collaboration,
            FutureSurfaceOperationClass::CollaborationEditProposal,
        ),
        (
            FutureSurfaceClass::RemoteWorkspace,
            FutureSurfaceOperationClass::RemoteWorkspaceProposal,
        ),
        (
            FutureSurfaceClass::Autonomy,
            FutureSurfaceOperationClass::AutonomousPlanProposal,
        ),
    ]
    .into_iter()
    .map(|(surface, operation)| {
        let gate = evaluate_future_surface_planning_gate(future_surface_input(
            surface,
            vec![
                FutureSurfaceOperationClass::MetadataProjection,
                FutureSurfaceOperationClass::ProposalOnlyEditOutput,
                operation,
            ],
        ));
        gate.validate().expect("future surface gate validates");
        gate
    })
    .collect::<Vec<_>>();

    assert!(gates.iter().all(|gate| {
        gate.classification == FutureSurfaceGateClassification::ProposalOnlyReady
            && gate.proposal_only_ready
            && gate.runtime_activation == FutureSurfaceRuntimeActivationState::NotEncoded
            && gate.blockers.is_empty()
            && gate.refusals.is_empty()
    }));

    let projection = future_surface_gate_projection_from_gates(
        "future-surface:p8-1",
        gates,
        TimestampMillis(2700),
        1,
    );
    assert_eq!(projection.gate_count, 5);
    assert_eq!(projection.proposal_only_ready_gate_count, 5);
    assert_eq!(projection.runtime_not_encoded_gate_count, 0);

    let value = serde_json::to_value(&projection).expect("serialize future surface projection");
    assert_eq!(value["proposal_only_ready_gate_count"], 5);
    assert_eq!(value["gates"][0]["runtime_activation"], "NotEncoded");
    assert_eq!(value["gates"][0]["classification"], "ProposalOnlyReady");

    let serialized = serde_json::to_string(&value).expect("stringify future surface projection");
    assert!(serialized.contains("Terminal"));
    assert!(serialized.contains("Plugin"));
    assert!(serialized.contains("Collaboration"));
    assert!(serialized.contains("RemoteWorkspace"));
    assert!(serialized.contains("Autonomy"));
    assert!(serialized.contains("workspace.mutation.proposal_only"));
    assert!(!serialized.contains("fn main"));
    assert!(!serialized.contains("raw prompt"));
    assert!(!serialized.contains("source_body"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("provider_payload"));
    assert!(!serialized.contains("terminal output"));
    assert!(!serialized.contains("command text"));
    assert!(!serialized.contains("full diff"));
    assert!(!serialized.contains("collaboration transcript"));
    assert!(!serialized.contains("remote file body"));
    assert!(!serialized.contains("model-generated prose"));
    assert!(!serialized.contains("runtime_started"));
    assert!(!serialized.contains("ApplyProposal"));

    let roundtrip: FutureSurfaceGateProjection =
        serde_json::from_value(value.clone()).expect("deserialize future surface projection");
    assert_eq!(
        roundtrip.gates[0].classification,
        FutureSurfaceGateClassification::ProposalOnlyReady
    );
    let mut missing = value;
    remove_required_field::<FutureSurfaceGateProjection>(&mut missing, "schema_version");
}

#[test]
fn dto_contracts_future_surface_gate_blockers_refusals_and_classifications_are_explicit() {
    let policy_required = evaluate_future_surface_planning_gate(FutureSurfacePlanningGateInput {
        adr_status: FutureSurfaceRequirementStatus::Missing,
        dependency_policy_status: FutureSurfaceRequirementStatus::Draft,
        threat_model_status: FutureSurfaceRequirementStatus::Present,
        ..future_surface_input(
            FutureSurfaceClass::Plugin,
            vec![FutureSurfaceOperationClass::ProposalOnlyEditOutput],
        )
    });
    assert_eq!(
        policy_required.classification,
        FutureSurfaceGateClassification::PolicyRequired
    );
    let policy_codes = policy_required
        .blockers
        .iter()
        .map(|reason| reason.reason_code.as_str())
        .collect::<Vec<_>>();
    assert!(policy_codes.contains(&"adr.required"));
    assert!(policy_codes.contains(&"dependency_policy.required"));
    assert!(policy_codes.contains(&"threat_model.required"));

    let contract_tests_required =
        evaluate_future_surface_planning_gate(FutureSurfacePlanningGateInput {
            contract_test_status: FutureSurfaceRequirementStatus::Missing,
            ..future_surface_input(
                FutureSurfaceClass::Terminal,
                vec![FutureSurfaceOperationClass::ProposalOnlyEditOutput],
            )
        });
    assert_eq!(
        contract_tests_required.classification,
        FutureSurfaceGateClassification::ContractTestsRequired
    );
    assert!(
        contract_tests_required
            .blockers
            .iter()
            .any(|reason| reason.reason_code == "contract_tests.required")
    );

    let trust_required = evaluate_future_surface_planning_gate(FutureSurfacePlanningGateInput {
        workspace_trust_state: WorkspaceTrustState::Untrusted,
        ..future_surface_input(
            FutureSurfaceClass::RemoteWorkspace,
            vec![FutureSurfaceOperationClass::RemoteWorkspaceProposal],
        )
    });
    assert_eq!(
        trust_required.classification,
        FutureSurfaceGateClassification::TrustRequired
    );
    assert!(
        trust_required
            .blockers
            .iter()
            .any(|reason| reason.reason_code == "workspace_trust.required")
    );

    let blocked = evaluate_future_surface_planning_gate(FutureSurfacePlanningGateInput {
        approval_available: false,
        checkpoint_available: false,
        rollback_available: false,
        proposal_only_mutation_available: false,
        ..future_surface_input(
            FutureSurfaceClass::Collaboration,
            vec![FutureSurfaceOperationClass::CollaborationEditProposal],
        )
    });
    assert_eq!(
        blocked.classification,
        FutureSurfaceGateClassification::Blocked
    );
    let blocker_codes = blocked
        .blockers
        .iter()
        .map(|reason| reason.reason_code.as_str())
        .collect::<Vec<_>>();
    assert!(blocker_codes.contains(&"approval.required"));
    assert!(blocker_codes.contains(&"checkpoint.required"));
    assert!(blocker_codes.contains(&"rollback.required"));
    assert!(blocker_codes.contains(&"proposal_only.required"));

    let refused = evaluate_future_surface_planning_gate(FutureSurfacePlanningGateInput {
        privacy_denied: true,
        permission_budget_denied: true,
        permission_budget_depleted: true,
        requested_operation_classes: vec![FutureSurfaceOperationClass::EditorMutation],
        ..future_surface_input(
            FutureSurfaceClass::Autonomy,
            vec![FutureSurfaceOperationClass::ProposalOnlyEditOutput],
        )
    });
    assert_eq!(
        refused.classification,
        FutureSurfaceGateClassification::Refused
    );
    let refusal_codes = refused
        .refusals
        .iter()
        .map(|reason| reason.reason_code.as_str())
        .collect::<Vec<_>>();
    assert!(refusal_codes.contains(&"privacy.denied"));
    assert!(refusal_codes.contains(&"budget.denied"));
    assert!(refusal_codes.contains(&"budget.depleted"));
    assert!(refusal_codes.contains(&"operation.denied"));
}

#[test]
fn dto_contracts_future_surface_runtime_activation_remains_not_encoded_for_all_surfaces() {
    let runtime_requests = [
        (
            FutureSurfaceClass::Terminal,
            FutureSurfaceOperationClass::RuntimeProcessLaunch,
        ),
        (
            FutureSurfaceClass::Plugin,
            FutureSurfaceOperationClass::RuntimePluginHost,
        ),
        (
            FutureSurfaceClass::Collaboration,
            FutureSurfaceOperationClass::RuntimeCollaborationSession,
        ),
        (
            FutureSurfaceClass::RemoteWorkspace,
            FutureSurfaceOperationClass::RuntimeRemoteSession,
        ),
        (
            FutureSurfaceClass::Autonomy,
            FutureSurfaceOperationClass::RuntimeAutonomousExecution,
        ),
    ];

    for (surface, runtime_operation) in runtime_requests {
        let gate = evaluate_future_surface_planning_gate(future_surface_input(
            surface,
            vec![runtime_operation],
        ));
        assert_eq!(
            gate.classification,
            FutureSurfaceGateClassification::RuntimeNotEncoded
        );
        assert_eq!(
            gate.runtime_activation,
            FutureSurfaceRuntimeActivationState::NotEncoded
        );
        assert!(!gate.proposal_only_ready);
        assert!(
            gate.blockers
                .iter()
                .any(|reason| reason.reason_code == "runtime.not_encoded")
        );
        gate.validate().expect("runtime-not-encoded gate validates");
    }
}

#[test]
fn dto_contracts_future_surface_gate_validation_rejects_raw_markers_and_invalid_core_ids() {
    let gate = evaluate_future_surface_planning_gate(future_surface_input(
        FutureSurfaceClass::Terminal,
        vec![FutureSurfaceOperationClass::TerminalCommandProposal],
    ));

    let mut zero_correlation = gate.clone();
    zero_correlation.correlation_id = CorrelationId(0);
    assert!(matches!(
        zero_correlation.validate(),
        Err(AssistedAiContractError::ZeroCorrelationId)
    ));

    let mut nil_causality = gate.clone();
    nil_causality.causality_id = CausalityId(Uuid::nil());
    assert!(matches!(
        nil_causality.validate(),
        Err(AssistedAiContractError::NilCausalityId)
    ));

    let mut raw_marker = gate;
    raw_marker
        .labels
        .push("terminal output provider_payload".to_string());
    assert!(matches!(
        raw_marker.validate(),
        Err(AssistedAiContractError::NonMetadataOnlyAuditRecord { .. })
    ));
}

#[test]
fn dto_contracts_collaboration_transport_envelope_serializes_metadata_and_operations() {
    let operation = CollaborationDocumentOperation {
        session_id: CollaborationSessionId(1001),
        operation_id: CollaborationOperationId(3001),
        author_participant_id: CollaborationParticipantId(2001),
        participant_sequence: 8,
        kind: CollaborationDocumentOperationKind::Replace {
            text: "bounded replacement".to_string(),
        },
        range: Some(TextRange::byte(10, 14)),
        preconditions: collaboration_preconditions(),
        undo_group: Some(UndoGroup {
            group_id: Uuid::parse_str("77777777-7777-7777-7777-777777777777").unwrap(),
            transaction_ids: vec![Uuid::parse_str("88888888-8888-8888-8888-888888888888").unwrap()],
        }),
        occurred_at: TimestampMillis(1700),
        schema_version: 1,
    };
    let envelope = CollaborationTransportEnvelope {
        session_id: CollaborationSessionId(1001),
        sender_participant_id: CollaborationParticipantId(2001),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        payload: CollaborationTransportPayload::Operation(Box::new(operation)),
        schema_version: 1,
    };

    let value = serde_json::to_value(&envelope).expect("collaboration envelope should serialize");

    assert_eq!(value["session_id"], json!(1001));
    assert_eq!(value["payload"]["Operation"]["operation_id"], json!(3001));
    assert_eq!(
        value["payload"]["Operation"]["preconditions"]["redaction_hints"],
        json!(["MetadataOnly"])
    );

    let roundtrip: CollaborationTransportEnvelope =
        serde_json::from_value(value.clone()).expect("collaboration envelope should round trip");
    assert_eq!(roundtrip.session_id, CollaborationSessionId(1001));

    let mut missing = value;
    remove_required_field::<CollaborationTransportEnvelope>(&mut missing, "schema_version");
}

#[test]
fn dto_contracts_collaboration_identity_metadata_rejects_zero_and_denied_values() {
    let valid = collaboration_preconditions();
    assert!(valid.has_valid_identity_metadata());

    let zero_correlation = CollaborationOperationPreconditions {
        correlation_id: CorrelationId(0),
        ..valid.clone()
    };
    assert!(!zero_correlation.has_valid_identity_metadata());

    let nil_causality = CollaborationOperationPreconditions {
        causality_id: CausalityId(Uuid::nil()),
        ..valid.clone()
    };
    assert!(!nil_causality.has_valid_identity_metadata());

    let denied = CollaborationOperationPreconditions {
        capability_decision: CapabilityDecision {
            granted: false,
            reason: Some("collaboration capability denied".to_string()),
            ..collaboration_capability_decision()
        },
        ..valid
    };
    assert!(!denied.has_valid_identity_metadata());
}

#[test]
fn dto_contracts_collaboration_shared_proposal_approval_links_policy_and_operations() {
    let approval = CollaborationSharedProposalApproval {
        session_id: CollaborationSessionId(1001),
        proposal_id: ProposalId(700),
        participant_id: CollaborationParticipantId(2001),
        disposition: CollaborationSharedProposalDisposition::Approved,
        capability_decision: collaboration_capability_decision(),
        applied_operation_ids: vec![CollaborationOperationId(3001)],
        denial_reason: None,
        schema_version: 1,
    };

    let value = serde_json::to_value(&approval).expect("shared approval should serialize");

    assert_eq!(value["proposal_id"], json!(700));
    assert_eq!(value["disposition"], json!("Approved"));
    assert_eq!(value["applied_operation_ids"], json!([3001]));
    assert_eq!(value["capability_decision"]["decision_id"], json!(901));

    let roundtrip: CollaborationSharedProposalApproval =
        serde_json::from_value(value).expect("shared approval should round trip");
    assert_eq!(
        roundtrip.applied_operation_ids,
        vec![CollaborationOperationId(3001)]
    );
}

#[test]
fn dto_contracts_collaboration_audit_records_are_metadata_only() {
    let audit = CollaborationAuditRecord {
        session_id: CollaborationSessionId(1001),
        operation_id: Some(CollaborationOperationId(3001)),
        proposal_id: Some(ProposalId(700)),
        event_sequence: EventSequence(42),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        retention_label: RetentionLabel::Audit,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        metadata_summary: "replace range 10..14, 19 bytes redacted".to_string(),
        schema_version: 1,
    };

    let serialized = serde_json::to_string(&audit).expect("audit record should serialize");

    assert!(serialized.contains("MetadataOnly"));
    assert!(serialized.contains("replace range"));
    assert!(!serialized.contains("bounded replacement"));
    assert!(!serialized.contains("source_text"));
}

#[test]
fn dto_contracts_collaboration_replay_manifest_is_metadata_only_and_audit_validated() {
    let manifest = CollaborationReplayManifest {
        session_id: CollaborationSessionId(1001),
        operation_ids: vec![CollaborationOperationId(3001)],
        participant_count: 2,
        acknowledgement_count: 1,
        causal_gap_count: 0,
        final_byte_count: 42,
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        event_sequence: EventSequence(7),
        retention_label: RetentionLabel::Audit,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let value = serde_json::to_value(&manifest).expect("manifest should serialize");

    assert_eq!(value["final_byte_count"], json!(42));
    assert!(!value.to_string().contains("bounded replacement"));

    let valid_audit = CollaborationAuditRecord {
        session_id: CollaborationSessionId(1001),
        operation_id: Some(CollaborationOperationId(3001)),
        proposal_id: None,
        event_sequence: EventSequence(8),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        retention_label: RetentionLabel::Audit,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        metadata_summary: "operation_count=1 byte_count=42".to_string(),
        schema_version: 1,
    };
    validate_collaboration_audit_record(&valid_audit).expect("metadata-only audit is valid");

    let invalid_audit = CollaborationAuditRecord {
        metadata_summary: "raw_source=fn main() {}".to_string(),
        ..valid_audit
    };
    assert!(validate_collaboration_audit_record(&invalid_audit).is_err());
}

fn remote_capability_decision(capability: &str) -> CapabilityDecision {
    CapabilityDecision {
        decision_id: CapabilityDecisionId(1701),
        granted: true,
        capability: CapabilityId(capability.to_string()),
        reason: None,
    }
}

fn remote_session_descriptor(
    state: RemoteWorkspaceLifecycleState,
) -> RemoteWorkspaceSessionDescriptor {
    RemoteWorkspaceSessionDescriptor {
        session_id: RemoteWorkspaceSessionId(7001),
        authority: RemoteAuthorityDescriptor {
            authority_id: RemoteAuthorityId(7101),
            authority_label: "edge-authority:hash".to_string(),
            workspace_id: WorkspaceId(11),
            trust_state: WorkspaceTrustState::Trusted,
            principal_id: PrincipalId("principal-remote".to_string()),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        agent: RemoteAgentDescriptor {
            agent_id: RemoteAgentId(7201),
            authority_id: RemoteAuthorityId(7101),
            agent_version: "devil-remote-test-agent/1".to_string(),
            runtime_enabled: true,
            schema_version: 1,
        },
        state,
        granted_capabilities: vec![
            RemoteCapabilityKind::Connect,
            RemoteCapabilityKind::FilesystemRead,
            RemoteCapabilityKind::FilesystemWrite,
        ],
        created_at: TimestampMillis(1700),
        last_heartbeat_at: Some(TimestampMillis(1800)),
        schema_version: 1,
    }
}

#[test]
fn dto_contracts_remote_session_activation_is_fail_closed_until_trusted_enabled_and_active() {
    let active = remote_session_descriptor(RemoteWorkspaceLifecycleState::Active);
    assert!(active.activation_is_policy_ready());

    let degraded = RemoteWorkspaceSessionDescriptor {
        state: RemoteWorkspaceLifecycleState::Degraded,
        ..active.clone()
    };
    assert!(!degraded.activation_is_policy_ready());

    let disabled = RemoteWorkspaceSessionDescriptor {
        agent: RemoteAgentDescriptor {
            runtime_enabled: false,
            ..active.agent.clone()
        },
        ..active.clone()
    };
    assert!(!disabled.activation_is_policy_ready());

    let untrusted = RemoteWorkspaceSessionDescriptor {
        authority: RemoteAuthorityDescriptor {
            trust_state: WorkspaceTrustState::Untrusted,
            ..active.authority.clone()
        },
        ..active
    };
    assert!(!untrusted.activation_is_policy_ready());
}

#[test]
fn dto_contracts_remote_transport_envelope_serializes_metadata_only_payloads() {
    let snapshot = RemoteFilesystemSnapshot {
        session_id: RemoteWorkspaceSessionId(7001),
        workspace_id: WorkspaceId(11),
        workspace_generation: WorkspaceGeneration(77),
        snapshot_id: SnapshotId(66),
        file_id: Some(FileId(33)),
        file_content_version: Some(FileContentVersion(44)),
        fingerprint: Some(fingerprint("remote-snapshot")),
        byte_len: Some(2048),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let envelope = RemoteTransportEnvelope {
        session_id: RemoteWorkspaceSessionId(7001),
        operation_id: RemoteOperationId(7301),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        event_sequence: EventSequence(42),
        principal_id: PrincipalId("principal-remote".to_string()),
        payload: RemoteTransportPayload::FilesystemSnapshot(snapshot),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    assert!(envelope.has_valid_event_identity());
    let value = serde_json::to_value(&envelope).expect("remote envelope should serialize");

    assert_eq!(value["session_id"], json!(7001));
    assert_eq!(
        value["payload"]["FilesystemSnapshot"]["byte_len"],
        json!(2048)
    );
    assert!(!value.to_string().contains("raw_source"));
    assert!(!value.to_string().contains("terminal_transcript"));

    let roundtrip: RemoteTransportEnvelope =
        serde_json::from_value(value.clone()).expect("remote envelope should round trip");
    assert_eq!(roundtrip.operation_id, RemoteOperationId(7301));

    let mut missing = value;
    remove_required_field::<RemoteTransportEnvelope>(&mut missing, "schema_version");
}

#[test]
fn dto_contracts_remote_write_preconditions_reject_missing_guards() {
    let valid = RemoteWritePreconditions {
        capability_decision: remote_capability_decision("remote.fs.write"),
        principal_id: PrincipalId("principal-remote".to_string()),
        expected_fingerprint: Some(fingerprint("expected")),
        file_content_version: FileContentVersion(44),
        workspace_generation: WorkspaceGeneration(77),
        buffer_version: Some(BufferVersion(55)),
        snapshot_id: SnapshotId(66),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
    };
    assert!(valid.has_required_write_guards());

    let zero_correlation = RemoteWritePreconditions {
        correlation_id: CorrelationId(0),
        ..valid.clone()
    };
    assert!(!zero_correlation.has_required_write_guards());

    let nil_causality = RemoteWritePreconditions {
        causality_id: CausalityId(Uuid::nil()),
        ..valid.clone()
    };
    assert!(!nil_causality.has_required_write_guards());

    let denied_capability = RemoteWritePreconditions {
        capability_decision: CapabilityDecision {
            granted: false,
            reason: Some("remote write denied".to_string()),
            ..remote_capability_decision("remote.fs.write")
        },
        ..valid
    };
    assert!(!denied_capability.has_required_write_guards());
}

#[test]
fn dto_contracts_remote_audit_records_require_metadata_only_redaction() {
    let valid = RemoteAuditRecord {
        session_id: RemoteWorkspaceSessionId(7001),
        operation_id: Some(RemoteOperationId(7301)),
        proposal_id: Some(ProposalId(700)),
        event_sequence: EventSequence(42),
        correlation_id: CorrelationId(901),
        causality_id: causality_id(),
        retention_label: RetentionLabel::Audit,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        metadata_summary: "remote write proposal_id=700 byte_len=2048".to_string(),
        schema_version: 1,
    };
    assert!(valid.is_metadata_only_valid());

    let serialized = serde_json::to_string(&valid).expect("remote audit should serialize");
    assert!(serialized.contains("MetadataOnly"));
    assert!(!serialized.contains("raw_source"));
    assert!(!serialized.contains("raw_transcript"));
    assert!(!serialized.contains("process_output"));

    let raw_allowed = RemoteAuditRecord {
        redaction_hints: vec![RedactionHint::None],
        ..valid.clone()
    };
    assert!(!raw_allowed.is_metadata_only_valid());

    let zero_sequence = RemoteAuditRecord {
        event_sequence: EventSequence(0),
        ..valid
    };
    assert!(!zero_sequence.is_metadata_only_valid());

    let raw_marker = RemoteAuditRecord {
        metadata_summary: "raw_source=fn main() {}".to_string(),
        ..zero_sequence
    };
    assert!(validate_remote_audit_record(&raw_marker).is_err());
}

#[test]
fn dto_contracts_phase4_runtime_surfaces_are_protocol_mediated() {
    let agent_src = include_str!("../../devil-agent/src/lib.rs");
    let tracker_src = include_str!("../../devil-tracker/src/lib.rs");
    let memory_src = include_str!("../../devil-memory/src/lib.rs");
    assert!(agent_src.contains("#![warn(missing_docs)]"));
    assert!(tracker_src.contains("#![warn(missing_docs)]"));
    assert!(memory_src.contains("#![warn(missing_docs)]"));
    assert!(agent_src.contains("devil_protocol"));
    assert!(tracker_src.contains("devil_protocol"));
    assert!(memory_src.contains("devil_protocol"));
    assert!(!agent_src.contains("WorkspaceActor"));
    assert!(!tracker_src.contains("WorkspaceActor"));
    assert!(!memory_src.contains("WorkspaceActor"));
    assert!(!agent_src.contains("EditorSession"));
    assert!(!tracker_src.contains("EditorSession"));
    assert!(!memory_src.contains("EditorSession"));

    let agent_manifest = include_str!("../../devil-agent/Cargo.toml");
    let tracker_manifest = include_str!("../../devil-tracker/Cargo.toml");
    let memory_manifest = include_str!("../../devil-memory/Cargo.toml");
    assert!(!agent_manifest.contains("devil-platform"));
    assert!(!agent_manifest.contains("devil-observability"));
    assert!(!agent_manifest.contains("devil-project"));
    assert!(!agent_manifest.contains("devil-editor"));
    assert!(!tracker_manifest.contains("devil-platform"));
    assert!(!tracker_manifest.contains("devil-observability"));
    assert!(!memory_manifest.contains("devil-platform"));
    assert!(!memory_manifest.contains("devil-observability"));
}
