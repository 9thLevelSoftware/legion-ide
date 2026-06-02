use devil_index::{
    DEFAULT_GRAMMAR_VERSION, DEFAULT_MODEL_VERSION, INDEX_SCHEMA_VERSION, IndexError,
    IndexWorkItem, IndexWorkKind, IndexingActor, LOCAL_RETRIEVAL_EMBEDDING_DIMENSIONS,
    LexicalFallbackParser, LexicalIndexer, ParserWorker, RETRIEVAL_CHUNK_SHA256_ALGORITHM,
    RepositoryDiscoveryImporter, RetrievalQuery, SemanticFabricScheduler,
    SemanticFabricSchedulingPolicy, SemanticIndex, SemanticSourceInputKind, SemanticUpsertOutcome,
    SourceDocument, StructuralRewriteFileInput, StructuralSearchQuery, SyntaxCacheEventKind,
    SyntaxTreeCache, WorkCompletionState, WorkPriority, build_rename_preview_payload,
    build_structural_rewrite_preview_payload, run_structural_search, semantic_cancellation_token,
};
use devil_protocol::{
    BufferId, BufferVersion, ByteRange, CancellationTokenId, CanonicalPath, CapabilityDecisionId,
    CapabilityId, CausalityId, CorrelationId, EditBatch, FileContentVersion, FileFingerprint,
    FileId, FileIdentity, LanguageId, LanguageServerId, LineIndexRange,
    LspConfiguredServerIdentity, LspContractValidationError, LspDiagnosticSummary,
    LspEditProposalConversionInput, LspLaunchDisposition, LspLaunchPolicyDecision,
    LspRequestCorrelation, PrincipalId, ProposalId, ProposalLifecycleState, ProposalPayload,
    ProposalPayloadKind, ProposalPrivacyLabel, ProposalTargetCoverage, ProposalTargetCoverageKind,
    ProposalTargetKind, ProposalVersionPreconditions, ProtocolDiagnostic,
    ProtocolDiagnosticSeverity, ProtocolTextRange, RedactionHint, SemanticCancellationReason,
    SemanticFabricDependencyHint, SemanticFabricInvalidationCause, SemanticFabricSchedulingAction,
    SemanticFabricSchedulingTrigger, SemanticFabricWorkSourceKind, SemanticFileFingerprintIdentity,
    SemanticFreshnessState, SemanticGrammarVersion, SemanticGraphRecordKind,
    SemanticMetadataSourceKind, SemanticModelVersion, SemanticPort, SemanticPrivacyScope,
    SemanticQueryFreshnessPolicy, SemanticQueryId, SemanticQueryKind, SemanticQueryRequest,
    SemanticQueryScope, SemanticQueryStatus, SemanticRequest, SemanticResponse,
    SnapshotChunkDescriptor, SnapshotConsumerKind, SnapshotId, SnapshotLeaseChunk,
    SnapshotLeaseDescriptor, TextCoordinate, TextEdit, TextRange, TimestampMillis,
    WorkspaceDiscoveryChangeKind, WorkspaceDiscoveryDecision, WorkspaceDiscoveryDelta,
    WorkspaceDiscoveryPathPolicyResult, WorkspaceDiscoveryPolicyDecision, WorkspaceDiscoveryRecord,
    WorkspaceDiscoverySkipReason, WorkspaceDiscoverySnapshot, WorkspaceDiscoveryTrustResult,
    WorkspaceEditProposalPayload, WorkspaceEditSourceKind, WorkspaceGeneration, WorkspaceId,
    WorkspaceRootId, WorkspaceTextEdit, WorkspaceTrustState,
    convert_lsp_edit_to_workspace_proposal, validate_lsp_edit_proposal_contract,
};
use devil_text::{DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES, TextSnapshot};
use uuid::Uuid;

fn document(path: &str, version: u64, generation: u64, text: &str) -> SourceDocument {
    document_with(
        path,
        FileId(11),
        LanguageId("rust".to_string()),
        version,
        generation,
        SemanticPrivacyScope::Workspace,
        text,
    )
}

fn document_with(
    path: &str,
    file_id: FileId,
    language_id: LanguageId,
    version: u64,
    generation: u64,
    privacy_scope: SemanticPrivacyScope,
    text: &str,
) -> SourceDocument {
    SourceDocument::with_versions(
        WorkspaceId(7),
        file_id,
        CanonicalPath(path.to_string()),
        language_id,
        FileContentVersion(version),
        WorkspaceGeneration(generation),
        Some(SnapshotId(100 + u128::from(version))),
        privacy_scope,
        text,
    )
}

fn token(number: u128) -> devil_protocol::SemanticCancellationToken {
    semantic_cancellation_token(
        CancellationTokenId(Uuid::from_u128(number)),
        WorkspaceId(7),
        Some(FileId(11)),
        None,
        None,
        Some(WorkspaceGeneration(1)),
        SemanticPrivacyScope::Workspace,
    )
}

fn work(
    number: u128,
    priority: WorkPriority,
    version: u64,
    generation: u64,
    text: &str,
) -> IndexWorkItem {
    IndexWorkItem::new(
        if priority == WorkPriority::LiveSnapshot {
            IndexWorkKind::LiveSnapshot
        } else {
            IndexWorkKind::BackgroundFile
        },
        priority,
        token(number),
        Some(document("/workspace/src/lib.rs", version, generation, text)),
    )
}

fn query(
    kind: SemanticQueryKind,
    text_hash: Option<FileFingerprint>,
    limit: u32,
) -> SemanticQueryRequest {
    SemanticQueryRequest {
        query_id: SemanticQueryId(Uuid::from_u128(9001)),
        kind,
        scope: SemanticQueryScope {
            workspace_id: WorkspaceId(7),
            file_ids: Vec::new(),
            paths: Vec::new(),
            language_ids: Vec::new(),
            privacy_scope: SemanticPrivacyScope::Workspace,
        },
        position: None,
        text_query_hash: text_hash,
        limit,
        cancellation_token: CancellationTokenId(Uuid::from_u128(42)),
        freshness_policy: SemanticQueryFreshnessPolicy::RequireFresh,
        correlation_id: CorrelationId(1),
        causality_id: CausalityId(Uuid::from_u128(43)),
        schema_version: INDEX_SCHEMA_VERSION,
    }
}

fn discovery_record(
    path: &str,
    decision: WorkspaceDiscoveryDecision,
    skip_reason: Option<WorkspaceDiscoverySkipReason>,
    change_kind: Option<WorkspaceDiscoveryChangeKind>,
    file_id: Option<FileId>,
) -> WorkspaceDiscoveryRecord {
    let content_hash = FileFingerprint {
        algorithm: "workspace-content-hash".to_string(),
        value: format!("hash:{path}"),
    };
    let identity = file_id.map(|file_id| FileIdentity {
        file_id,
        workspace_id: WorkspaceId(55),
        canonical_path: CanonicalPath(path.to_string()),
        content_version: FileContentVersion(7),
        content_hash: Some(content_hash.value.clone()),
    });
    WorkspaceDiscoveryRecord {
        schema_version: 1,
        workspace_id: Some(WorkspaceId(55)),
        workspace_root_id: None,
        workspace_generation: WorkspaceGeneration(3),
        identity,
        path: Some(CanonicalPath(path.to_string())),
        display_path: Some(path.replace("C:/repo/", "")),
        metadata: None,
        policy: WorkspaceDiscoveryPolicyDecision {
            decision,
            skip_reason,
            path_policy: if skip_reason == Some(WorkspaceDiscoverySkipReason::External) {
                WorkspaceDiscoveryPathPolicyResult::External
            } else if skip_reason == Some(WorkspaceDiscoverySkipReason::PolicyDenied) {
                WorkspaceDiscoveryPathPolicyResult::WorkspaceDenied
            } else {
                WorkspaceDiscoveryPathPolicyResult::WorkspaceAllowed
            },
            trust: WorkspaceDiscoveryTrustResult::Trusted,
            generated: skip_reason == Some(WorkspaceDiscoverySkipReason::Generated),
            binary: skip_reason == Some(WorkspaceDiscoverySkipReason::Binary),
            vendored: skip_reason == Some(WorkspaceDiscoverySkipReason::Vendored),
            oversized: skip_reason == Some(WorkspaceDiscoverySkipReason::Oversized),
            metadata_only: decision != WorkspaceDiscoveryDecision::ContentAllowed,
        },
        language_hint: Some(LanguageId("rust".to_string())),
        privacy_scope: SemanticPrivacyScope::Workspace,
        content_fingerprint: Some(content_hash.clone()),
        content_hash: if decision == WorkspaceDiscoveryDecision::ContentAllowed {
            Some(content_hash)
        } else {
            None
        },
        change_kind,
    }
}

fn snapshot_identity(snapshot: &TextSnapshot, file_id: FileId) -> SemanticFileFingerprintIdentity {
    SemanticFileFingerprintIdentity {
        workspace_id: WorkspaceId(7),
        file_id,
        canonical_path: CanonicalPath(format!("/workspace/src/{}.rs", file_id.0)),
        file_content_version: FileContentVersion(1),
        workspace_generation: WorkspaceGeneration(1),
        content_hash: FileFingerprint {
            algorithm: "devil-text-snapshot-content-hash-v1".to_string(),
            value: snapshot.content_hash().to_string(),
        },
        disk_fingerprint: None,
        byte_len: Some(snapshot.len() as u64),
        modified_at: None,
        privacy_scope: SemanticPrivacyScope::Workspace,
        schema_version: INDEX_SCHEMA_VERSION,
    }
}

fn lease_chunks_from_snapshot(snapshot: &TextSnapshot) -> Vec<SnapshotLeaseChunk> {
    snapshot
        .chunk_descriptors()
        .iter()
        .map(|chunk| {
            let lease = SnapshotLeaseDescriptor {
                lease_id: Uuid::from_u128(0xabc0 + chunk.ordinal as u128),
                buffer_id: BufferId(77),
                snapshot_id: snapshot.snapshot_id(),
                buffer_version: snapshot.buffer_version(),
                consumer_kind: SnapshotConsumerKind::Index,
                expires_at: TimestampMillis(999_999),
                chunk_count: snapshot.chunk_descriptors().len() as u32,
                schema_version: INDEX_SCHEMA_VERSION,
            };
            let descriptor = SnapshotChunkDescriptor {
                snapshot_id: snapshot.snapshot_id(),
                chunk_index: chunk.ordinal as u32,
                byte_range: ByteRange::new(chunk.start_byte as u64, chunk.end_byte as u64),
                line_range: LineIndexRange {
                    start: chunk.start_line as u32,
                    end: chunk.end_line.saturating_add(1) as u32,
                },
                byte_len: chunk.byte_len as u64,
                chunk_hash: FileFingerprint {
                    algorithm: "devil-text-chunk-sha256-v1".to_string(),
                    value: chunk.hash.clone(),
                },
                schema_version: INDEX_SCHEMA_VERSION,
            };
            SnapshotLeaseChunk {
                lease,
                chunk: descriptor,
                text: snapshot
                    .chunk_text(chunk.ordinal)
                    .expect("bounded chunk text should be available"),
                schema_version: INDEX_SCHEMA_VERSION,
            }
        })
        .collect()
}

fn fabric_scheduler(capacity: u32) -> SemanticFabricScheduler {
    SemanticFabricScheduler::new(SemanticFabricSchedulingPolicy::new(
        WorkspaceGeneration(1),
        SemanticPrivacyScope::Workspace,
        capacity,
    ))
}

fn fabric_scheduler_for_generation(
    generation: WorkspaceGeneration,
    capacity: u32,
) -> SemanticFabricScheduler {
    SemanticFabricScheduler::new(SemanticFabricSchedulingPolicy::new(
        generation,
        SemanticPrivacyScope::Workspace,
        capacity,
    ))
}

fn metadata_record_for(document: &SourceDocument) -> devil_protocol::SemanticMetadataRecord {
    LexicalIndexer::new()
        .index_document(
            document,
            SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
            SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
        )
        .to_semantic_metadata_record()
}

fn protocol_range() -> ProtocolTextRange {
    ProtocolTextRange {
        start: TextCoordinate {
            line: 0,
            character: 3,
            byte_offset: Some(3),
            utf16_offset: Some(3),
        },
        end: TextCoordinate {
            line: 0,
            character: 7,
            byte_offset: Some(7),
            utf16_offset: Some(7),
        },
    }
}

fn protocol_diagnostic(code: &str) -> ProtocolDiagnostic {
    ProtocolDiagnostic {
        code: code.to_string(),
        message: format!("metadata-only diagnostic {code}"),
        severity: ProtocolDiagnosticSeverity::Info,
        path: None,
        range: Some(protocol_range()),
    }
}

fn lsp_diagnostic_summary_for(identity: &SemanticFileFingerprintIdentity) -> LspDiagnosticSummary {
    LspDiagnosticSummary {
        workspace_id: identity.workspace_id,
        file_id: identity.file_id,
        snapshot_id: SnapshotId(207),
        buffer_version: BufferVersion(17),
        content_hash: Some(identity.content_hash.clone()),
        diagnostic_count: 2,
        error_count: 1,
        warning_count: 1,
        information_count: 0,
        hint_count: 0,
        ranges: vec![protocol_range()],
        diagnostic_hashes: vec![FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "diagnostic-hash".to_string(),
        }],
        source_hashes: vec![FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "source-label-hash".to_string(),
        }],
        freshness: SemanticFreshnessState::Stale,
        privacy_scope: SemanticPrivacyScope::Workspace,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: INDEX_SCHEMA_VERSION,
    }
}

fn lsp_identity() -> LspConfiguredServerIdentity {
    LspConfiguredServerIdentity {
        server_id: LanguageServerId(7),
        workspace_id: WorkspaceId(55),
        root_id: Some(WorkspaceRootId(5)),
        language_id: LanguageId("rust".to_string()),
        display_name: "rust-analyzer".to_string(),
        command_hash: FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "command-label-hash".to_string(),
        },
        args_hash: Some(FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "args-hash".to_string(),
        }),
        env_hash: Some(FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "env-hash".to_string(),
        }),
        cwd_hash: Some(FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "cwd-hash".to_string(),
        }),
        settings_hash: Some(FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "settings-hash".to_string(),
        }),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: INDEX_SCHEMA_VERSION,
    }
}

fn lsp_posture(
    workspace_trust_state: WorkspaceTrustState,
    privacy_scope_allowed: bool,
) -> devil_protocol::LspWorkspaceTrustPosture {
    devil_protocol::LspWorkspaceTrustPosture {
        workspace_id: WorkspaceId(55),
        workspace_trust_state,
        privacy_scope: SemanticPrivacyScope::Workspace,
        privacy_scope_allowed,
        required_capability: CapabilityId("process.spawn".to_string()),
        decision_id: Some(CapabilityDecisionId(99)),
        diagnostics: vec![protocol_diagnostic("lsp.posture")],
        schema_version: INDEX_SCHEMA_VERSION,
    }
}

fn lsp_preconditions(identity: &SemanticFileFingerprintIdentity) -> ProposalVersionPreconditions {
    ProposalVersionPreconditions {
        file_version: Some(identity.file_content_version),
        buffer_version: Some(BufferVersion(17)),
        snapshot_id: Some(SnapshotId(207)),
        generation: Some(identity.workspace_generation),
        file_content_version: Some(identity.file_content_version),
        workspace_generation: Some(identity.workspace_generation),
        expected_fingerprint: Some(identity.content_hash.clone()),
        expected_file_length: identity.byte_len,
        expected_modified_at: identity.modified_at,
    }
}

fn lsp_request_correlation(identity: &SemanticFileFingerprintIdentity) -> LspRequestCorrelation {
    LspRequestCorrelation {
        request_id: devil_protocol::LspRequestId(Uuid::from_u128(4307)),
        server_id: LanguageServerId(7),
        workspace_id: identity.workspace_id,
        file_id: Some(identity.file_id),
        snapshot_id: Some(SnapshotId(207)),
        buffer_version: Some(BufferVersion(17)),
        correlation_id: CorrelationId(4307),
        causality_id: CausalityId(Uuid::from_u128(4307)),
        cancellation_token: Some(CancellationTokenId(Uuid::from_u128(4308))),
        privacy_scope: SemanticPrivacyScope::Workspace,
        issued_at: TimestampMillis(4307),
        schema_version: INDEX_SCHEMA_VERSION,
    }
}

fn lsp_conversion_input(
    identity: SemanticFileFingerprintIdentity,
) -> LspEditProposalConversionInput {
    let preconditions = lsp_preconditions(&identity);
    LspEditProposalConversionInput {
        proposal_id: ProposalId(4307),
        principal: PrincipalId("principal-p4-3".to_string()),
        capability: CapabilityId("fs.write".to_string()),
        request: lsp_request_correlation(&identity),
        workspace_edit: WorkspaceEditProposalPayload {
            workspace_id: identity.workspace_id,
            edit_id: Uuid::from_u128(4309),
            title: "rename through proposal".to_string(),
            source: WorkspaceEditSourceKind::LspRename,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: vec![devil_protocol::ProposalAffectedTarget {
                    target_id: "p4-3-open-buffer".to_string(),
                    kind: ProposalTargetKind::OpenBuffer,
                    workspace_id: Some(identity.workspace_id),
                    file_id: Some(identity.file_id),
                    buffer_id: Some(BufferId(17)),
                    path: Some(identity.canonical_path.clone()),
                    terminal_session_id: None,
                    plugin_id: None,
                    remote_authority: None,
                    collaboration_session_id: None,
                    byte_ranges: vec![ByteRange::new(3, 7)],
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                }],
                omitted_target_count: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            file_edits: vec![WorkspaceTextEdit {
                file: FileIdentity {
                    file_id: identity.file_id,
                    workspace_id: identity.workspace_id,
                    canonical_path: identity.canonical_path,
                    content_version: identity.file_content_version,
                    content_hash: Some(identity.content_hash.value),
                },
                buffer_id: Some(BufferId(17)),
                edits: EditBatch {
                    edits: vec![TextEdit {
                        range: TextRange::byte(3, 7),
                        replacement: "renamed_lsp".to_string(),
                    }],
                },
                preconditions: preconditions.clone(),
            }],
            file_operations: Vec::new(),
            required_capability: CapabilityId("fs.write".to_string()),
            diagnostics: vec![protocol_diagnostic("lsp.edit.converted")],
            schema_version: INDEX_SCHEMA_VERSION,
        },
        preconditions,
        lifecycle_state: ProposalLifecycleState::Created,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        preview: devil_protocol::PreviewSummary {
            summary: "proposal-mediated LSP edit".to_string(),
            details: vec!["metadata-only preview".to_string()],
        },
        expires_at: Some(TimestampMillis(5307)),
        created_at: TimestampMillis(4307),
        diagnostics: vec![protocol_diagnostic("lsp.conversion")],
        schema_version: INDEX_SCHEMA_VERSION,
    }
}

#[test]
fn queue_backpressure_rejects_low_priority_and_displaces_lower_priority_work() {
    let mut actor = IndexingActor::new(1);
    let first = actor
        .submit(work(1, WorkPriority::BackgroundScan, 1, 1, "fn slow() {}"))
        .expect("initial work should fit bounded queue");
    assert_eq!(first.pending_len, 1);

    let rejected = actor
        .submit(IndexWorkItem::new(
            IndexWorkKind::BackgroundFile,
            WorkPriority::BackgroundScan,
            token(2),
            Some(SourceDocument::with_versions(
                WorkspaceId(7),
                FileId(12),
                CanonicalPath("/workspace/src/other.rs".to_string()),
                LanguageId("rust".to_string()),
                FileContentVersion(1),
                WorkspaceGeneration(1),
                Some(SnapshotId(202)),
                SemanticPrivacyScope::Workspace,
                "fn slow_again() {}",
            )),
        ))
        .expect_err("same-priority work should be rejected under pressure");
    assert!(matches!(
        rejected,
        IndexError::QueueBackpressure {
            capacity: 1,
            pending_len: 1,
            priority: WorkPriority::BackgroundScan
        }
    ));

    let accepted = actor
        .submit(work(3, WorkPriority::LiveSnapshot, 3, 2, "fn fast() {}"))
        .expect("live work should displace lower-priority queued work");
    assert_eq!(accepted.pending_len, 1);
    assert!(
        accepted
            .cancellations
            .iter()
            .any(|ack| ack.reason == SemanticCancellationReason::SnapshotSuperseded)
    );
    assert_eq!(
        actor.pending_tokens(),
        vec![CancellationTokenId(Uuid::from_u128(3))]
    );
}

#[test]
fn priority_ordering_starts_highest_priority_before_fifo_background_work() {
    let mut actor = IndexingActor::new(4);
    actor
        .submit(work(
            1,
            WorkPriority::BackgroundScan,
            1,
            1,
            "fn background() {}",
        ))
        .unwrap();
    actor
        .submit(IndexWorkItem::new(
            IndexWorkKind::SemanticQuery,
            WorkPriority::Foreground,
            token(2),
            Some(SourceDocument::with_versions(
                WorkspaceId(7),
                FileId(12),
                CanonicalPath("/workspace/src/query.rs".to_string()),
                LanguageId("rust".to_string()),
                FileContentVersion(1),
                WorkspaceGeneration(1),
                Some(SnapshotId(200)),
                SemanticPrivacyScope::Workspace,
                "fn foreground() {}",
            )),
        ))
        .unwrap();
    actor
        .submit(IndexWorkItem::new(
            IndexWorkKind::BackgroundFile,
            WorkPriority::Normal,
            token(3),
            Some(SourceDocument::with_versions(
                WorkspaceId(7),
                FileId(13),
                CanonicalPath("/workspace/src/normal.rs".to_string()),
                LanguageId("rust".to_string()),
                FileContentVersion(1),
                WorkspaceGeneration(1),
                Some(SnapshotId(300)),
                SemanticPrivacyScope::Workspace,
                "fn normal() {}",
            )),
        ))
        .unwrap();

    let started = actor.start_next().expect("queued work should start");
    assert_eq!(started.item.priority, WorkPriority::Foreground);
    assert_eq!(
        started.item.cancellation.token_id,
        CancellationTokenId(Uuid::from_u128(2))
    );
}

#[test]
fn indexing_actor_exposes_semantic_port_for_planning_queries_and_cancellation() {
    let mut actor = IndexingActor::new(2);
    actor
        .submit(work(
            1,
            WorkPriority::LiveSnapshot,
            1,
            1,
            "pub fn semantic_port_symbol() {}",
        ))
        .unwrap();
    let report = actor.execute_next().unwrap().unwrap();
    assert_eq!(report.state, WorkCompletionState::Applied);

    let SemanticResponse::Query(response) = actor
        .handle(SemanticRequest::Query(query(
            SemanticQueryKind::SymbolLookup,
            None,
            10,
        )))
        .expect("semantic port query should be served by actor-owned index")
    else {
        panic!("semantic port returned unexpected response");
    };
    assert_eq!(response.status, SemanticQueryStatus::Fresh);
    assert!(
        response
            .results
            .iter()
            .any(|result| result.label == "semantic_port_symbol")
    );

    let document = document(
        "/workspace/src/port_plan.rs",
        2,
        1,
        "pub fn semantic_port_plan() {}",
    );
    let request = fabric_scheduler(2).request_from_source_document(
        &document,
        SemanticFabricSchedulingTrigger::RecentEdit,
        None,
        token(2),
        CorrelationId(2),
        CausalityId(Uuid::from_u128(2)),
    );
    let SemanticResponse::SchedulePlan(plan) = actor
        .handle(SemanticRequest::PlanJobs {
            requests: vec![request],
            correlation_id: CorrelationId(3),
            causality_id: CausalityId(Uuid::from_u128(3)),
        })
        .expect("semantic port should plan jobs through metadata-only scheduler")
    else {
        panic!("semantic port returned unexpected response");
    };
    assert_eq!(plan.capacity, 2);
    assert_eq!(plan.decisions.len(), 1);

    let SemanticResponse::Cancelled(cancelled) = actor
        .handle(SemanticRequest::Cancel(token(3)))
        .expect("semantic port should accept cancellation metadata")
    else {
        panic!("semantic port returned unexpected response");
    };
    assert_eq!(cancelled.token_id, CancellationTokenId(Uuid::from_u128(3)));
}

#[test]
fn cancellation_acknowledges_queued_and_in_flight_work() {
    let mut actor = IndexingActor::new(3);
    actor
        .submit(work(1, WorkPriority::Normal, 1, 1, "fn queued() {}"))
        .unwrap();
    let queued_ack = actor
        .cancel(
            CancellationTokenId(Uuid::from_u128(1)),
            SemanticCancellationReason::UserCancelled,
        )
        .expect("queued cancellation should be acknowledged");
    assert!(queued_ack.removed_from_queue);
    assert!(!queued_ack.was_in_flight);
    assert_eq!(actor.pending_len(), 0);

    actor
        .submit(work(2, WorkPriority::Normal, 2, 1, "fn running() {}"))
        .unwrap();
    let started = actor.start_next().unwrap();
    let running_ack = actor
        .cancel(
            CancellationTokenId(Uuid::from_u128(2)),
            SemanticCancellationReason::Shutdown,
        )
        .expect("in-flight cancellation should be acknowledged");
    assert!(running_ack.was_in_flight);
    let report = actor.complete_started(started).unwrap();
    assert_eq!(report.state, WorkCompletionState::Cancelled);
    assert_eq!(
        report.cancellation_ack.unwrap().reason,
        SemanticCancellationReason::Shutdown
    );
}

#[test]
fn live_snapshot_supersedes_slower_background_work_by_generation_and_hash() {
    let mut actor = IndexingActor::new(4);
    actor
        .submit(work(1, WorkPriority::BackgroundScan, 1, 1, "fn stale() {}"))
        .unwrap();
    let started = actor.start_next().unwrap();

    let supersession = actor
        .submit(work(2, WorkPriority::LiveSnapshot, 2, 2, "fn fresh() {}"))
        .unwrap();
    assert!(supersession.cancellations.iter().any(
        |ack| ack.was_in_flight && ack.reason == SemanticCancellationReason::SnapshotSuperseded
    ));

    let stale_report = actor.complete_started(started).unwrap();
    assert_eq!(stale_report.state, WorkCompletionState::Cancelled);

    let fresh_report = actor.execute_next().unwrap().unwrap();
    assert_eq!(fresh_report.state, WorkCompletionState::Applied);
    assert!(
        actor
            .index()
            .symbols()
            .iter()
            .any(|symbol| symbol.display_name.as_deref() == Some("fresh"))
    );
}

#[test]
fn generation_bump_with_reset_content_version_supersedes_old_work() {
    let mut actor = IndexingActor::new(4);
    actor
        .submit(work(
            1,
            WorkPriority::Normal,
            5,
            1,
            "fn stale_generation() {}",
        ))
        .unwrap();
    let started = actor.start_next().unwrap();

    let supersession = actor
        .submit(work(
            2,
            WorkPriority::Normal,
            1,
            2,
            "fn fresh_generation() {}",
        ))
        .unwrap();
    assert!(supersession.cancellations.iter().any(
        |ack| ack.was_in_flight && ack.reason == SemanticCancellationReason::SnapshotSuperseded
    ));

    let stale_report = actor.complete_started(started).unwrap();
    assert_eq!(stale_report.state, WorkCompletionState::Cancelled);

    let fresh_report = actor.execute_next().unwrap().unwrap();
    assert_eq!(fresh_report.state, WorkCompletionState::Applied);
    assert!(
        actor
            .index()
            .symbols()
            .iter()
            .any(|symbol| symbol.display_name.as_deref() == Some("fresh_generation"))
    );
    assert!(
        !actor
            .index()
            .symbols()
            .iter()
            .any(|symbol| symbol.display_name.as_deref() == Some("stale_generation"))
    );
}

#[test]
fn repository_discovery_importer_accepts_workspace_dtos_and_never_scans_paths() {
    let snapshot = WorkspaceDiscoverySnapshot {
        schema_version: 1,
        workspace_id: WorkspaceId(55),
        workspace_root_id: None,
        workspace_generation: WorkspaceGeneration(3),
        captured_at: TimestampMillis(1234),
        records: vec![
            discovery_record(
                "C:/repo/src/lib.rs",
                WorkspaceDiscoveryDecision::ContentAllowed,
                None,
                Some(WorkspaceDiscoveryChangeKind::Added),
                Some(FileId(1)),
            ),
            discovery_record(
                "C:/repo/target/generated.rs",
                WorkspaceDiscoveryDecision::MetadataOnly,
                Some(WorkspaceDiscoverySkipReason::Generated),
                Some(WorkspaceDiscoveryChangeKind::PolicyChanged),
                Some(FileId(2)),
            ),
            discovery_record(
                "C:/repo/secret.rs",
                WorkspaceDiscoveryDecision::Excluded,
                Some(WorkspaceDiscoverySkipReason::PolicyDenied),
                Some(WorkspaceDiscoveryChangeKind::PolicyChanged),
                Some(FileId(3)),
            ),
        ],
        diagnostics: Vec::new(),
    };

    let outcome = RepositoryDiscoveryImporter::new().ingest_snapshot(&snapshot);

    assert_eq!(outcome.content_records.len(), 1);
    assert_eq!(outcome.metadata_only_records.len(), 1);
    assert_eq!(outcome.excluded_records.len(), 1);
    assert_eq!(outcome.invalidated_file_ids, vec![FileId(3)]);
    assert_eq!(
        outcome.content_records[0]
            .identity
            .as_ref()
            .unwrap()
            .file_id,
        FileId(1)
    );
}

#[test]
fn repository_discovery_importer_invalidates_deleted_records_from_workspace_delta() {
    let delta = WorkspaceDiscoveryDelta {
        schema_version: 1,
        workspace_id: WorkspaceId(55),
        workspace_generation: WorkspaceGeneration(4),
        sequence: devil_protocol::EventSequence(9),
        records: vec![discovery_record(
            "C:/repo/src/lib.rs",
            WorkspaceDiscoveryDecision::Excluded,
            Some(WorkspaceDiscoverySkipReason::Deleted),
            Some(WorkspaceDiscoveryChangeKind::Deleted),
            Some(FileId(1)),
        )],
        diagnostics: Vec::new(),
    };

    let outcome = RepositoryDiscoveryImporter::new().ingest_delta(&delta);

    assert!(outcome.content_records.is_empty());
    assert_eq!(outcome.excluded_records.len(), 1);
    assert_eq!(outcome.invalidated_file_ids, vec![FileId(1)]);
}

#[test]
fn lexical_indexer_extracts_symbol_maps_graph_records_and_invalidation_keys() {
    let document = document(
        "/workspace/src/lib.rs",
        4,
        5,
        "use crate::dep;\n// owner: platform\npub struct Widget { field: String }\npub fn test_widget() {\n    helper();\n    Widget {};\n}\nfn helper() { /* TODO diagnose */ }\n",
    );
    let file_index = LexicalIndexer::new().index_document(
        &document,
        SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
        SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
    );

    let names = file_index
        .symbols
        .iter()
        .filter_map(|symbol| symbol.display_name.as_deref())
        .collect::<Vec<_>>();
    assert!(names.contains(&"Widget"));
    assert!(names.contains(&"test_widget"));
    assert!(names.contains(&"helper"));
    assert!(
        file_index
            .symbols
            .iter()
            .find(|symbol| symbol.display_name.as_deref() == Some("Widget"))
            .unwrap()
            .reference_ranges
            .iter()
            .any(|range| range.start.line == 5)
    );

    let kinds = file_index
        .graph_records
        .iter()
        .map(|record| record.kind)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&SemanticGraphRecordKind::Symbol));
    assert!(kinds.contains(&SemanticGraphRecordKind::Reference));
    assert!(kinds.contains(&SemanticGraphRecordKind::Import));
    assert!(kinds.contains(&SemanticGraphRecordKind::Export));
    assert!(kinds.contains(&SemanticGraphRecordKind::CallEdge));
    assert!(kinds.contains(&SemanticGraphRecordKind::TypeRelation));
    assert!(kinds.contains(&SemanticGraphRecordKind::TestLink));
    assert!(kinds.contains(&SemanticGraphRecordKind::DiagnosticLink));
    assert!(kinds.contains(&SemanticGraphRecordKind::OwnershipMetadata));
    assert_eq!(
        file_index.symbols[0].invalidation_key.workspace_generation,
        WorkspaceGeneration(5)
    );
    assert_eq!(
        file_index.syntax_tree.declaration_count,
        file_index.symbols.len()
    );
}

#[test]
fn large_snapshot_descriptor_first_indexing_uses_chunks_without_full_source() {
    let repeated_line = "fn descriptor_only_large_marker() {}\n";
    let repeat_count = (DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES / repeated_line.len()) + 128;
    let snapshot = TextSnapshot::try_new(repeated_line.repeat(repeat_count))
        .expect("large descriptor snapshot should construct");
    assert!(snapshot.len() > DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES);

    let document = SourceDocument::from_text_snapshot_descriptors(
        WorkspaceId(7),
        FileId(77),
        CanonicalPath("/workspace/src/large.rs".to_string()),
        LanguageId("rust".to_string()),
        FileContentVersion(1),
        WorkspaceGeneration(1),
        SemanticPrivacyScope::Workspace,
        &snapshot,
    );
    assert_eq!(
        document.source_kind(),
        SemanticSourceInputKind::DescriptorOnly
    );
    assert!(!document.uses_bounded_full_text());
    assert!(!document.source_descriptor().chunks.is_empty());

    let file_index = LexicalIndexer::new().index_document(
        &document,
        SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
        SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
    );

    assert_eq!(
        file_index.source_kind,
        SemanticSourceInputKind::DescriptorOnly
    );
    assert_eq!(
        file_index.source_freshness.state,
        SemanticFreshnessState::Partial
    );
    assert!(file_index.symbols.is_empty());
    assert!(!file_index.source_chunks.is_empty());
    assert!(file_index.source_chunks.iter().all(|chunk| {
        chunk.byte_len > 0 && !chunk.chunk_hash.value.is_empty() && chunk.lease_id.is_none()
    }));
    assert_eq!(
        file_index.source_ranges.len(),
        file_index.source_chunks.len()
    );
    assert!(!format!("{:?}", file_index.source_chunks).contains("descriptor_only_large_marker"));

    let mut cache = SyntaxTreeCache::new();
    let outcome = cache
        .get_or_parse(
            &LexicalFallbackParser::new(),
            devil_index::ParseRequest {
                document,
                grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
                model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
            },
        )
        .expect("descriptor-only parse should produce metadata-only cache record");
    assert_eq!(
        outcome.syntax_tree.cache_key.descriptor.source_kind,
        SemanticSourceInputKind::DescriptorOnly
    );
    assert_eq!(
        outcome.syntax_tree.cache_key.privacy_scope,
        SemanticPrivacyScope::Workspace
    );
    assert_eq!(
        outcome.syntax_tree.cache_key.descriptor.chunks.len(),
        file_index.source_chunks.len()
    );
    assert!(
        outcome
            .syntax_tree
            .cache_key
            .descriptor
            .chunks
            .iter()
            .all(|chunk| {
                chunk.byte_len > 0 && !chunk.chunk_hash.value.is_empty() && !chunk.lease_present
            })
    );
    assert!(
        outcome
            .syntax_tree
            .cache_key
            .descriptor
            .ranges
            .iter()
            .all(|range| range.end > range.start)
    );
    assert!(
        !format!("{:?}", outcome.syntax_tree.cache_key).contains("descriptor_only_large_marker")
    );
}

#[test]
fn semantic_metadata_record_for_large_descriptor_persists_only_hashes_ranges_and_metadata() {
    let repeated_line = "fn metadata_large_marker_must_not_persist() {}\n";
    let repeat_count = (DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES / repeated_line.len()) + 128;
    let snapshot = TextSnapshot::try_new(repeated_line.repeat(repeat_count))
        .expect("large descriptor snapshot should construct");
    let document = SourceDocument::from_text_snapshot_descriptors(
        WorkspaceId(7),
        FileId(177),
        CanonicalPath("/workspace/src/large_metadata.rs".to_string()),
        LanguageId("rust".to_string()),
        FileContentVersion(1),
        WorkspaceGeneration(1),
        SemanticPrivacyScope::Workspace,
        &snapshot,
    );

    let file_index = LexicalIndexer::new().index_document(
        &document,
        SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
        SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
    );
    let metadata = file_index.to_semantic_metadata_record();

    assert_eq!(
        metadata.freshness_key.descriptor.source_kind,
        SemanticMetadataSourceKind::DescriptorOnly
    );
    assert!(!metadata.freshness_key.descriptor.chunks.is_empty());
    assert_eq!(
        metadata.freshness_key.descriptor.ranges.len(),
        metadata.freshness_key.descriptor.chunks.len()
    );
    assert!(
        metadata
            .freshness_key
            .descriptor
            .chunks
            .iter()
            .all(|chunk| chunk.byte_len > 0 && !chunk.chunk_hash.value.is_empty())
    );
    let serialized = format!("{metadata:?}");
    assert!(!serialized.contains("metadata_large_marker_must_not_persist"));
    assert!(!serialized.contains(repeated_line.trim()));
}

#[test]
fn semantic_metadata_freshness_key_separates_schema_parser_grammar_language_and_descriptor() {
    let rust_document = document(
        "/workspace/src/lib.rs",
        8,
        9,
        "pub fn separated_symbol() {}\n",
    );
    let rust_index = LexicalIndexer::new().index_document(
        &rust_document,
        SemanticGrammarVersion("grammar-v1".to_string()),
        SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
    );
    let rust_metadata = rust_index.to_semantic_metadata_record();

    let mut grammar_changed = rust_metadata.freshness_key.clone();
    grammar_changed.grammar_version = Some(SemanticGrammarVersion("grammar-v2".to_string()));
    assert_ne!(rust_metadata.freshness_key, grammar_changed);

    let mut parser_changed = rust_metadata.freshness_key.clone();
    parser_changed.parser_version = "parser-v2".to_string();
    assert_ne!(rust_metadata.freshness_key, parser_changed);

    let mut schema_changed = rust_metadata.freshness_key.clone();
    schema_changed.schema_version = INDEX_SCHEMA_VERSION + 1;
    assert_ne!(rust_metadata.freshness_key, schema_changed);

    let typescript_document = document_with(
        "/workspace/src/lib.ts",
        FileId(12),
        LanguageId("typescript".to_string()),
        8,
        9,
        SemanticPrivacyScope::Workspace,
        "pub fn separated_symbol() {}\n",
    );
    let typescript_index = LexicalIndexer::new().index_document(
        &typescript_document,
        SemanticGrammarVersion("grammar-v1".to_string()),
        SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
    );
    assert_ne!(
        rust_metadata.freshness_key,
        typescript_index.to_semantic_metadata_record().freshness_key
    );

    let snapshot =
        TextSnapshot::try_new("pub fn separated_symbol() {}\n").expect("snapshot should construct");
    let descriptor_document = SourceDocument::from_text_snapshot_descriptors(
        WorkspaceId(7),
        FileId(11),
        CanonicalPath("/workspace/src/lib.rs".to_string()),
        LanguageId("rust".to_string()),
        FileContentVersion(8),
        WorkspaceGeneration(9),
        SemanticPrivacyScope::Workspace,
        &snapshot,
    );
    let descriptor_index = LexicalIndexer::new().index_document(
        &descriptor_document,
        SemanticGrammarVersion("grammar-v1".to_string()),
        SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
    );
    assert_ne!(
        rust_metadata.freshness_key.descriptor,
        descriptor_index
            .to_semantic_metadata_record()
            .freshness_key
            .descriptor
    );
}

#[test]
fn semantic_fabric_jobs_are_built_from_workspace_descriptor_and_metadata_records_only() {
    let scheduler = fabric_scheduler_for_generation(WorkspaceGeneration(3), 8);
    let snapshot = WorkspaceDiscoverySnapshot {
        schema_version: 1,
        workspace_id: WorkspaceId(55),
        workspace_root_id: None,
        workspace_generation: WorkspaceGeneration(3),
        captured_at: TimestampMillis(1234),
        records: vec![discovery_record(
            "C:/repo/src/fabric.rs",
            WorkspaceDiscoveryDecision::ContentAllowed,
            None,
            Some(WorkspaceDiscoveryChangeKind::Added),
            Some(FileId(91)),
        )],
        diagnostics: Vec::new(),
    };
    let imported = RepositoryDiscoveryImporter::new().ingest_snapshot(&snapshot);
    let discovery_request = scheduler
        .request_from_discovery_record(
            &imported.content_records[0],
            SemanticFabricSchedulingTrigger::WorkspaceDiscovery,
            None,
            token(91),
            CorrelationId(91),
            CausalityId(Uuid::from_u128(91)),
        )
        .expect("workspace-authored discovery should build a semantic fabric job");
    assert_eq!(discovery_request.workspace_id, WorkspaceId(55));
    assert_eq!(discovery_request.file_id, FileId(91));
    assert!(discovery_request.privacy.metadata_only);

    let descriptor_document = SourceDocument::from_text_snapshot_descriptors(
        WorkspaceId(7),
        FileId(92),
        CanonicalPath("/workspace/src/fabric_descriptor.rs".to_string()),
        LanguageId("rust".to_string()),
        FileContentVersion(1),
        WorkspaceGeneration(1),
        SemanticPrivacyScope::Workspace,
        &TextSnapshot::try_new("fn fabric_descriptor_body_must_not_schedule() {}\n")
            .expect("descriptor snapshot should construct"),
    );
    let descriptor_request = fabric_scheduler(8).request_from_source_document(
        &descriptor_document,
        SemanticFabricSchedulingTrigger::ForegroundViewport,
        None,
        token(92),
        CorrelationId(92),
        CausalityId(Uuid::from_u128(92)),
    );
    assert!(!descriptor_request.descriptor.chunks.is_empty());
    assert!(
        descriptor_request
            .descriptor
            .ranges
            .iter()
            .all(|range| range.end > range.start)
    );

    let metadata_record = metadata_record_for(&descriptor_document);
    let metadata_request = fabric_scheduler(8).request_from_metadata_record(
        &metadata_record,
        SemanticFabricSchedulingTrigger::Maintenance,
        token(93),
        CorrelationId(93),
        CausalityId(Uuid::from_u128(93)),
    );
    let requests_debug = format!("{discovery_request:?}{descriptor_request:?}{metadata_request:?}");
    assert!(!requests_debug.contains("fabric_descriptor_body_must_not_schedule"));
    assert!(!requests_debug.contains("fn fabric_descriptor_body"));
}

#[test]
fn semantic_fabric_stale_freshness_metadata_lowers_or_rejects_scheduling() {
    let scheduler = fabric_scheduler(8);
    let document = document_with(
        "/workspace/src/fabric_stale.rs",
        FileId(94),
        LanguageId("rust".to_string()),
        1,
        1,
        SemanticPrivacyScope::Workspace,
        "fn stale_fabric() {}\n",
    );
    let metadata = metadata_record_for(&document);
    let base_request = scheduler.request_from_source_document(
        &document,
        SemanticFabricSchedulingTrigger::ForegroundViewport,
        Some(&metadata),
        token(94),
        CorrelationId(94),
        CausalityId(Uuid::from_u128(94)),
    );
    let fresh_plan = scheduler.plan(
        vec![base_request.clone()],
        CorrelationId(194),
        CausalityId(Uuid::from_u128(194)),
    );
    assert_eq!(
        fresh_plan.decisions[0].action,
        SemanticFabricSchedulingAction::Coalesce
    );

    let mut stale_cases = Vec::new();
    let mut privacy = base_request.clone();
    privacy
        .persisted_freshness_key
        .as_mut()
        .unwrap()
        .privacy_scope = SemanticPrivacyScope::Project;
    stale_cases.push((
        privacy,
        SemanticFabricInvalidationCause::PrivacyScopeChanged,
    ));
    let mut generation = base_request.clone();
    generation
        .persisted_freshness_key
        .as_mut()
        .unwrap()
        .workspace_generation = WorkspaceGeneration(0);
    stale_cases.push((
        generation,
        SemanticFabricInvalidationCause::WorkspaceGenerationChanged,
    ));
    let mut schema = base_request.clone();
    schema
        .persisted_freshness_key
        .as_mut()
        .unwrap()
        .schema_version = 99;
    stale_cases.push((
        schema,
        SemanticFabricInvalidationCause::SchemaVersionChanged,
    ));
    let mut parser = base_request.clone();
    parser
        .persisted_freshness_key
        .as_mut()
        .unwrap()
        .parser_version = "parser-v0".to_string();
    stale_cases.push((
        parser,
        SemanticFabricInvalidationCause::ParserVersionChanged,
    ));
    let mut model = base_request.clone();
    model
        .persisted_freshness_key
        .as_mut()
        .unwrap()
        .model_version = Some(SemanticModelVersion("model-v0".to_string()));
    stale_cases.push((model, SemanticFabricInvalidationCause::ModelVersionChanged));
    let mut grammar = base_request.clone();
    grammar
        .persisted_freshness_key
        .as_mut()
        .unwrap()
        .grammar_version = Some(SemanticGrammarVersion("grammar-v0".to_string()));
    stale_cases.push((
        grammar,
        SemanticFabricInvalidationCause::GrammarVersionChanged,
    ));
    let mut language = base_request.clone();
    language
        .persisted_freshness_key
        .as_mut()
        .unwrap()
        .language_id = LanguageId("typescript".to_string());
    stale_cases.push((language, SemanticFabricInvalidationCause::LanguageChanged));
    let mut descriptor = base_request.clone();
    descriptor
        .persisted_freshness_key
        .as_mut()
        .unwrap()
        .descriptor
        .schema_version = 99;
    stale_cases.push((
        descriptor,
        SemanticFabricInvalidationCause::DescriptorIdentityChanged,
    ));
    let mut content = base_request;
    content
        .persisted_freshness_key
        .as_mut()
        .unwrap()
        .content_hash = FileFingerprint {
        algorithm: "sha256".to_string(),
        value: "different".to_string(),
    };
    stale_cases.push((content, SemanticFabricInvalidationCause::ContentHashChanged));

    for (request, expected_cause) in stale_cases {
        let plan = scheduler.plan(
            vec![request],
            CorrelationId(195),
            CausalityId(Uuid::from_u128(195)),
        );
        let decision = &plan.decisions[0];
        assert_ne!(decision.action, SemanticFabricSchedulingAction::Coalesce);
        assert!(decision.invalidation_causes.contains(&expected_cause));
        assert!(matches!(
            decision.action,
            SemanticFabricSchedulingAction::Refresh
                | SemanticFabricSchedulingAction::Reindex
                | SemanticFabricSchedulingAction::Reject
        ));
    }
}

#[test]
fn semantic_fabric_priority_is_deterministic_metadata_only_and_bounded() {
    let scheduler = fabric_scheduler(2);
    let viewport_doc = document_with(
        "/workspace/src/viewport.rs",
        FileId(95),
        LanguageId("rust".to_string()),
        1,
        1,
        SemanticPrivacyScope::Workspace,
        "fn viewport_priority_must_not_leak() {}\n",
    );
    let edit_doc = document_with(
        "/workspace/src/edit.rs",
        FileId(96),
        LanguageId("rust".to_string()),
        1,
        1,
        SemanticPrivacyScope::Workspace,
        "fn edit_priority_must_not_leak() {}\n",
    );
    let background_doc = document_with(
        "/workspace/src/background.rs",
        FileId(97),
        LanguageId("rust".to_string()),
        1,
        1,
        SemanticPrivacyScope::Workspace,
        "fn background_priority_must_not_leak() {}\n",
    );
    let mut dependency_request = scheduler.request_from_source_document(
        &viewport_doc,
        SemanticFabricSchedulingTrigger::DependencyHint,
        None,
        token(95),
        CorrelationId(95),
        CausalityId(Uuid::from_u128(95)),
    );
    dependency_request
        .dependency_hints
        .push(SemanticFabricDependencyHint {
            file_id: FileId(96),
            label_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "dep".to_string(),
            },
            confidence_basis_points: 9000,
            schema_version: INDEX_SCHEMA_VERSION,
        });
    let requests = vec![
        scheduler.request_from_source_document(
            &background_doc,
            SemanticFabricSchedulingTrigger::BackgroundCrawl,
            None,
            token(97),
            CorrelationId(97),
            CausalityId(Uuid::from_u128(97)),
        ),
        dependency_request,
        scheduler.request_from_source_document(
            &edit_doc,
            SemanticFabricSchedulingTrigger::RecentEdit,
            None,
            token(96),
            CorrelationId(96),
            CausalityId(Uuid::from_u128(96)),
        ),
    ];
    let first = scheduler.plan(
        requests.clone(),
        CorrelationId(196),
        CausalityId(Uuid::from_u128(196)),
    );
    let second = scheduler.plan(
        requests,
        CorrelationId(196),
        CausalityId(Uuid::from_u128(196)),
    );
    assert_eq!(first.decisions, second.decisions);
    assert_eq!(first.admitted_count, 2);
    assert_eq!(first.decisions[0].priority_score, 1000);
    assert_eq!(first.decisions[1].priority_score, 725);
    assert_eq!(
        first.decisions[2].action,
        SemanticFabricSchedulingAction::Reject
    );
    assert!(
        first.decisions[2]
            .invalidation_causes
            .contains(&SemanticFabricInvalidationCause::QueuePressure)
    );
    let debug = format!("{first:?}");
    assert!(!debug.contains("viewport_priority_must_not_leak"));
    assert!(!debug.contains("edit_priority_must_not_leak"));
    assert!(!debug.contains("background_priority_must_not_leak"));
}

#[test]
fn semantic_fabric_large_descriptor_jobs_carry_only_chunk_hashes_ranges_and_lease_metadata() {
    let repeated_line = "fn fabric_large_marker_must_not_schedule_body() {}\n";
    let repeat_count = (DEFAULT_FULL_CACHE_BYTE_BUDGET_BYTES / repeated_line.len()) + 128;
    let snapshot = TextSnapshot::try_new(repeated_line.repeat(repeat_count))
        .expect("large fabric snapshot should construct");
    let chunks = lease_chunks_from_snapshot(&snapshot);
    let document = SourceDocument::from_snapshot_lease_chunks(
        snapshot_identity(&snapshot, FileId(98)),
        LanguageId("rust".to_string()),
        chunks,
    );
    let request = fabric_scheduler(8).request_from_source_document(
        &document,
        SemanticFabricSchedulingTrigger::RecentEdit,
        None,
        token(98),
        CorrelationId(98),
        CausalityId(Uuid::from_u128(98)),
    );
    assert!(!request.descriptor.chunks.is_empty());
    assert!(request.descriptor.chunks.iter().all(|chunk| {
        chunk.lease_present
            && chunk.byte_range.end > chunk.byte_range.start
            && !chunk.chunk_hash.value.is_empty()
    }));
    assert!(
        request
            .descriptor
            .ranges
            .iter()
            .all(|range| range.end > range.start)
    );
    let debug = format!("{request:?}");
    assert!(!debug.contains("fabric_large_marker_must_not_schedule_body"));
    assert!(!debug.contains(repeated_line.trim()));
}

#[test]
fn semantic_fabric_scheduler_does_not_start_lsp_or_external_processes() {
    let scheduler = fabric_scheduler(4);
    let document = document_with(
        "/workspace/src/lsp_gate.rs",
        FileId(99),
        LanguageId("rust".to_string()),
        1,
        1,
        SemanticPrivacyScope::Workspace,
        "fn lsp_gate_body_must_not_run() {}\n",
    );
    let request = scheduler.request_from_source_document(
        &document,
        SemanticFabricSchedulingTrigger::LspEnrichment,
        None,
        token(99),
        CorrelationId(99),
        CausalityId(Uuid::from_u128(99)),
    );
    let plan = scheduler.plan(
        vec![request],
        CorrelationId(199),
        CausalityId(Uuid::from_u128(199)),
    );
    assert_eq!(plan.decisions.len(), 1);
    assert!(matches!(
        plan.decisions[0].action,
        SemanticFabricSchedulingAction::Schedule | SemanticFabricSchedulingAction::Reindex
    ));
    let debug = format!("{plan:?}");
    assert!(!debug.contains("lsp_gate_body_must_not_run"));
    assert!(!debug.contains("process"));
    assert!(!debug.contains("JsonRpc"));
}

#[test]
fn p4_3_semantic_fabric_lsp_contract_path_is_metadata_only_non_runtime_and_proposal_only() {
    let scheduler = fabric_scheduler_for_generation(WorkspaceGeneration(3), 8);
    let discovery_snapshot = WorkspaceDiscoverySnapshot {
        schema_version: 1,
        workspace_id: WorkspaceId(55),
        workspace_root_id: Some(WorkspaceRootId(5)),
        workspace_generation: WorkspaceGeneration(3),
        captured_at: TimestampMillis(3333),
        records: vec![discovery_record(
            "C:/repo/src/p4_3.rs",
            WorkspaceDiscoveryDecision::ContentAllowed,
            None,
            Some(WorkspaceDiscoveryChangeKind::Added),
            Some(FileId(1043)),
        )],
        diagnostics: Vec::new(),
    };
    let imported = RepositoryDiscoveryImporter::new().ingest_snapshot(&discovery_snapshot);
    let discovery_request = scheduler
        .request_from_discovery_record(
            &imported.content_records[0],
            SemanticFabricSchedulingTrigger::WorkspaceDiscovery,
            None,
            token(1043),
            CorrelationId(4301),
            CausalityId(Uuid::from_u128(4301)),
        )
        .expect("workspace-authored discovery metadata should schedule semantic work");
    assert_eq!(
        discovery_request.source_kind,
        SemanticFabricWorkSourceKind::WorkspaceDiscovery
    );
    assert!(discovery_request.privacy.metadata_only);

    let source_document = document_with(
        "C:/repo/src/p4_3.rs",
        FileId(1043),
        LanguageId("rust".to_string()),
        7,
        3,
        SemanticPrivacyScope::Workspace,
        "fn p4_3_body_must_not_leak_into_contract_metadata() { println!(\"secret\"); }\n",
    );
    let persisted_metadata = metadata_record_for(&source_document);
    let lsp_summary = lsp_diagnostic_summary_for(&source_document.identity);
    let lsp_refresh_request = scheduler
        .request_from_lsp_diagnostic_summary(
            &lsp_summary,
            source_document.language_id.clone(),
            &source_document.identity,
            Some(&persisted_metadata),
            token(1044),
            CorrelationId(4302),
            CausalityId(Uuid::from_u128(4302)),
        )
        .expect("normalized LSP metadata should become semantic fabric refresh metadata");
    assert_eq!(
        lsp_refresh_request.source_kind,
        SemanticFabricWorkSourceKind::LspDtoMetadata
    );
    assert_eq!(
        lsp_refresh_request.trigger,
        SemanticFabricSchedulingTrigger::LspEnrichment
    );
    assert!(lsp_refresh_request.privacy.metadata_only);

    let plan = scheduler.plan(
        vec![discovery_request.clone(), lsp_refresh_request.clone()],
        CorrelationId(4303),
        CausalityId(Uuid::from_u128(4303)),
    );
    assert_eq!(plan.decisions.len(), 2);
    assert!(plan.decisions.iter().all(|decision| decision.metadata_only));
    assert!(plan.decisions.iter().any(|decision| {
        matches!(
            decision.action,
            SemanticFabricSchedulingAction::Schedule
                | SemanticFabricSchedulingAction::Refresh
                | SemanticFabricSchedulingAction::Reindex
                | SemanticFabricSchedulingAction::Coalesce
        )
    }));

    let untrusted_launch = LspLaunchPolicyDecision::evaluate(
        lsp_identity(),
        lsp_posture(WorkspaceTrustState::Untrusted, true),
        true,
        CorrelationId(4304),
        CausalityId(Uuid::from_u128(4304)),
        vec![protocol_diagnostic("lsp.launch.untrusted")],
        INDEX_SCHEMA_VERSION,
    );
    assert_eq!(
        untrusted_launch.disposition,
        LspLaunchDisposition::DisabledUntrustedWorkspace
    );
    assert!(!untrusted_launch.process_launch_allowed);

    let privacy_denied_launch = LspLaunchPolicyDecision::evaluate(
        lsp_identity(),
        lsp_posture(WorkspaceTrustState::Trusted, false),
        true,
        CorrelationId(4305),
        CausalityId(Uuid::from_u128(4305)),
        vec![protocol_diagnostic("lsp.launch.privacy")],
        INDEX_SCHEMA_VERSION,
    );
    assert_eq!(
        privacy_denied_launch.disposition,
        LspLaunchDisposition::DisabledPrivacyDenied
    );
    assert!(!privacy_denied_launch.process_launch_allowed);

    let deferred_launch = LspLaunchPolicyDecision::evaluate(
        lsp_identity(),
        lsp_posture(WorkspaceTrustState::Trusted, true),
        false,
        CorrelationId(4306),
        CausalityId(Uuid::from_u128(4306)),
        Vec::new(),
        INDEX_SCHEMA_VERSION,
    );
    assert_eq!(
        deferred_launch.disposition,
        LspLaunchDisposition::RuntimeActivationDeferred
    );
    assert!(!deferred_launch.process_launch_allowed);

    let proposal = convert_lsp_edit_to_workspace_proposal(lsp_conversion_input(
        source_document.identity.clone(),
    ))
    .expect("LSP edit output should convert only into a workspace proposal");
    assert_eq!(proposal.correlation_id, CorrelationId(4307));
    assert_eq!(proposal.preconditions.snapshot_id, Some(SnapshotId(207)));
    assert_eq!(
        proposal.preconditions.workspace_generation,
        Some(WorkspaceGeneration(3))
    );
    match &proposal.payload {
        ProposalPayload::WorkspaceEdit(payload) => {
            assert_eq!(payload.source, WorkspaceEditSourceKind::LspRename);
            assert_eq!(payload.file_edits.len(), 1);
            assert_eq!(
                payload.file_edits[0].edits.edits[0].replacement,
                "renamed_lsp"
            );
        }
        other => panic!("LSP mutation output must be proposal-mediated, got {other:?}"),
    }

    let mut missing_precondition = lsp_conversion_input(source_document.identity.clone());
    missing_precondition.preconditions.snapshot_id = None;
    assert_eq!(
        validate_lsp_edit_proposal_contract(&missing_precondition),
        Err(LspContractValidationError::MissingPrecondition)
    );
    let mut nil_causality = lsp_conversion_input(source_document.identity.clone());
    nil_causality.request.causality_id = CausalityId(Uuid::nil());
    assert_eq!(
        validate_lsp_edit_proposal_contract(&nil_causality),
        Err(LspContractValidationError::NilCausalityId)
    );
    let mut privacy_denied_edit = lsp_conversion_input(source_document.identity);
    privacy_denied_edit.request.privacy_scope = SemanticPrivacyScope::MetadataOnly;
    assert_eq!(
        validate_lsp_edit_proposal_contract(&privacy_denied_edit),
        Err(LspContractValidationError::PrivacyDenied)
    );

    let contract_debug = format!(
        "{discovery_request:?}{lsp_refresh_request:?}{plan:?}{untrusted_launch:?}{privacy_denied_launch:?}{deferred_launch:?}{proposal:?}"
    );
    for forbidden in [
        "p4_3_body_must_not_leak_into_contract_metadata",
        "println!",
        "secret",
        "chunk text",
        "prompt",
        "provider payload",
        "terminal output",
        "full diff",
        "reconstructed file body",
        "std::process",
        "process::Command",
        "thread::spawn",
        "worker loop",
        "JsonRpc",
        "RegisterServer",
        "OpenDocument",
        "TerminalRequest::Launch",
        "plugin runtime",
        "remote runtime",
        "collaboration runtime",
    ] {
        assert!(
            !contract_debug.contains(forbidden),
            "P4.3 contract path leaked forbidden runtime or payload marker `{forbidden}`"
        );
    }
}

#[test]
fn small_snapshot_full_text_indexing_is_explicit_bounded_optimization() {
    let snapshot = TextSnapshot::try_new("pub fn bounded_small() {}\n")
        .expect("small snapshot should construct");
    let document = SourceDocument::from_text_snapshot(
        WorkspaceId(7),
        FileId(78),
        CanonicalPath("/workspace/src/small.rs".to_string()),
        LanguageId("rust".to_string()),
        FileContentVersion(1),
        WorkspaceGeneration(1),
        SemanticPrivacyScope::Workspace,
        &snapshot,
    )
    .expect("small snapshot full-text optimization should be allowed");
    assert_eq!(
        document.source_kind(),
        SemanticSourceInputKind::BoundedFullText
    );
    assert!(document.uses_bounded_full_text());

    let file_index = LexicalIndexer::new().index_document(
        &document,
        SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
        SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
    );

    assert_eq!(
        file_index.source_kind,
        SemanticSourceInputKind::BoundedFullText
    );
    assert_eq!(
        file_index.source_freshness.state,
        SemanticFreshnessState::Fresh
    );
    assert!(file_index.source_chunks.iter().all(|chunk| {
        chunk.snapshot_id == snapshot.snapshot_id() && !chunk.chunk_hash.value.is_empty()
    }));
    assert!(
        file_index
            .symbols
            .iter()
            .any(|symbol| symbol.display_name.as_deref() == Some("bounded_small"))
    );
}

#[test]
fn lease_chunk_indexing_records_hashes_ranges_and_not_source_bodies() {
    let source = "// private_body_marker should never be stored as source\npub fn chunked_symbol() { helper(); }\nfn helper() {}\n";
    let snapshot = TextSnapshot::try_new(source).expect("snapshot should construct");
    let chunks = lease_chunks_from_snapshot(&snapshot);
    let first_chunk_hash = chunks[0].chunk.chunk_hash.clone();
    let identity = snapshot_identity(&snapshot, FileId(79));
    let document = SourceDocument::from_snapshot_lease_chunks(
        identity.clone(),
        LanguageId("rust".to_string()),
        chunks,
    );
    assert_eq!(document.source_kind(), SemanticSourceInputKind::LeaseChunks);
    assert!(!document.uses_bounded_full_text());

    let file_index = LexicalIndexer::new().index_document(
        &document,
        SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
        SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
    );

    assert_eq!(file_index.identity.file_id, identity.file_id);
    assert_eq!(file_index.source_kind, SemanticSourceInputKind::LeaseChunks);
    assert_eq!(
        file_index.source_freshness.state,
        SemanticFreshnessState::Fresh
    );
    assert_eq!(file_index.source_chunks[0].chunk_hash, first_chunk_hash);
    assert!(
        file_index
            .source_chunks
            .iter()
            .all(|chunk| chunk.lease_id.is_some() && chunk.byte_range.end > chunk.byte_range.start)
    );
    assert!(
        file_index
            .symbols
            .iter()
            .any(|symbol| symbol.display_name.as_deref() == Some("chunked_symbol"))
    );
    assert!(
        file_index
            .symbols
            .iter()
            .all(|symbol| symbol.declaration_range.is_some()
                && symbol.invalidation_key.content_hash == identity.content_hash)
    );
    assert!(!format!("{:?}", file_index.source_chunks).contains("private_body_marker"));
    assert!(!format!("{:?}", file_index.graph_records).contains("private_body_marker"));
}

#[test]
fn syntax_cache_keys_by_content_hash_language_and_grammar_version_without_collision() {
    let document = document("/workspace/src/cache.rs", 1, 1, "fn cached() {}\n");
    let typescript_document = document_with(
        "/workspace/src/cache.ts",
        FileId(12),
        LanguageId("typescript".to_string()),
        1,
        1,
        SemanticPrivacyScope::Workspace,
        "fn cached() {}\n",
    );
    let mut cache = SyntaxTreeCache::new();
    let parser = LexicalFallbackParser::new();
    let grammar_one = SemanticGrammarVersion("grammar-one".to_string());
    let grammar_two = SemanticGrammarVersion("grammar-two".to_string());

    let first = cache
        .get_or_parse(
            &parser,
            devil_index::ParseRequest {
                document: document.clone(),
                grammar_version: grammar_one.clone(),
                model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
            },
        )
        .unwrap();
    let second = cache
        .get_or_parse(
            &parser,
            devil_index::ParseRequest {
                document: document.clone(),
                grammar_version: grammar_one.clone(),
                model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
            },
        )
        .unwrap();
    assert_eq!(first.syntax_tree.cache_key, second.syntax_tree.cache_key);
    assert_eq!(cache.len(), 1);

    let typescript = cache
        .get_or_parse(
            &parser,
            devil_index::ParseRequest {
                document: typescript_document,
                grammar_version: grammar_one.clone(),
                model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
            },
        )
        .unwrap();
    assert_eq!(
        first.syntax_tree.cache_key.content_hash,
        typescript.syntax_tree.cache_key.content_hash
    );
    assert_ne!(
        first.syntax_tree.cache_key.language_id,
        typescript.syntax_tree.cache_key.language_id
    );
    assert_eq!(cache.len(), 2);

    cache
        .get_or_parse(
            &parser,
            devil_index::ParseRequest {
                document,
                grammar_version: grammar_two.clone(),
                model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
            },
        )
        .unwrap();
    assert_eq!(cache.len(), 3);
    assert_eq!(cache.invalidate_grammar(&grammar_one), 2);
    assert_eq!(cache.len(), 1);
    assert!(
        cache
            .events()
            .iter()
            .any(|event| event.kind == SyntaxCacheEventKind::Hit)
    );
}

#[test]
fn privacy_scope_changes_separate_syntax_cache_and_replace_semantic_records() {
    let workspace_doc = document_with(
        "/workspace/src/privacy.rs",
        FileId(31),
        LanguageId("rust".to_string()),
        1,
        1,
        SemanticPrivacyScope::Workspace,
        "pub fn privacy_marker() {}\n",
    );
    let metadata_doc = document_with(
        "/workspace/src/privacy.rs",
        FileId(31),
        LanguageId("rust".to_string()),
        1,
        1,
        SemanticPrivacyScope::MetadataOnly,
        "pub fn privacy_marker() {}\n",
    );
    assert_eq!(
        workspace_doc.identity.content_hash,
        metadata_doc.identity.content_hash
    );

    let mut cache = SyntaxTreeCache::new();
    let parser = LexicalFallbackParser::new();
    let workspace_outcome = cache
        .get_or_parse(
            &parser,
            devil_index::ParseRequest {
                document: workspace_doc,
                grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
                model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
            },
        )
        .unwrap();
    let metadata_outcome = cache
        .get_or_parse(
            &parser,
            devil_index::ParseRequest {
                document: metadata_doc,
                grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
                model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
            },
        )
        .unwrap();

    assert_ne!(
        workspace_outcome.syntax_tree.cache_key,
        metadata_outcome.syntax_tree.cache_key
    );
    assert_eq!(cache.len(), 2);
    assert_eq!(
        metadata_outcome.syntax_tree.cache_key.privacy_scope,
        SemanticPrivacyScope::MetadataOnly
    );

    let mut index = SemanticIndex::new();
    assert_eq!(
        index.upsert(workspace_outcome.file_index),
        SemanticUpsertOutcome::Applied
    );
    assert!(
        index
            .symbols()
            .iter()
            .any(|symbol| symbol.display_name.as_deref() == Some("privacy_marker"))
    );
    assert_eq!(
        index.upsert(metadata_outcome.file_index),
        SemanticUpsertOutcome::Replaced
    );
    assert!(index.symbols().iter().all(|symbol| {
        symbol.invalidation_key.privacy_scope == SemanticPrivacyScope::MetadataOnly
            && symbol.display_name.is_none()
    }));
}

#[test]
fn workspace_generation_changes_separate_syntax_cache_and_reject_stale_semantic_records() {
    let old_doc = document_with(
        "/workspace/src/generation.rs",
        FileId(32),
        LanguageId("rust".to_string()),
        1,
        1,
        SemanticPrivacyScope::Workspace,
        "fn same_generation_hash() {}\n",
    );
    let new_doc = document_with(
        "/workspace/src/generation.rs",
        FileId(32),
        LanguageId("rust".to_string()),
        1,
        2,
        SemanticPrivacyScope::Workspace,
        "fn same_generation_hash() {}\n",
    );
    assert_eq!(old_doc.identity.content_hash, new_doc.identity.content_hash);

    let mut cache = SyntaxTreeCache::new();
    let parser = LexicalFallbackParser::new();
    let old_outcome = cache
        .get_or_parse(
            &parser,
            devil_index::ParseRequest {
                document: old_doc,
                grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
                model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
            },
        )
        .unwrap();
    let new_outcome = cache
        .get_or_parse(
            &parser,
            devil_index::ParseRequest {
                document: new_doc,
                grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
                model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
            },
        )
        .unwrap();

    assert_ne!(
        old_outcome.syntax_tree.cache_key,
        new_outcome.syntax_tree.cache_key
    );
    assert_eq!(cache.len(), 2);

    let mut index = SemanticIndex::new();
    assert_eq!(
        index.upsert(new_outcome.file_index),
        SemanticUpsertOutcome::Applied
    );
    assert_eq!(
        index.upsert(old_outcome.file_index),
        SemanticUpsertOutcome::IgnoredStale
    );
    assert!(
        index
            .symbols()
            .iter()
            .all(|symbol| symbol.invalidation_key.workspace_generation == WorkspaceGeneration(2))
    );
}

#[test]
fn schema_and_model_version_changes_separate_cache_entries_and_semantic_keys() {
    let base_doc = document_with(
        "/workspace/src/schema.rs",
        FileId(33),
        LanguageId("rust".to_string()),
        1,
        1,
        SemanticPrivacyScope::Workspace,
        "fn schema_marker() {}\n",
    );
    let mut schema_doc = base_doc.clone();
    schema_doc.identity.schema_version = INDEX_SCHEMA_VERSION + 1;
    let parser = LexicalFallbackParser::new();
    let mut cache = SyntaxTreeCache::new();
    let model_one = SemanticModelVersion("semantic-ranking-metadata-v1".to_string());
    let model_two = SemanticModelVersion("semantic-ranking-metadata-v2".to_string());

    let base = cache
        .get_or_parse(
            &parser,
            devil_index::ParseRequest {
                document: base_doc.clone(),
                grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
                model_version: model_one.clone(),
            },
        )
        .unwrap();
    let model_changed = cache
        .get_or_parse(
            &parser,
            devil_index::ParseRequest {
                document: base_doc,
                grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
                model_version: model_two,
            },
        )
        .unwrap();
    let schema_changed = cache
        .get_or_parse(
            &parser,
            devil_index::ParseRequest {
                document: schema_doc,
                grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
                model_version: model_one,
            },
        )
        .unwrap();

    assert_ne!(
        base.syntax_tree.cache_key,
        model_changed.syntax_tree.cache_key
    );
    assert_ne!(
        base.syntax_tree.cache_key,
        schema_changed.syntax_tree.cache_key
    );
    assert_eq!(cache.len(), 3);
    assert_eq!(
        schema_changed
            .file_index
            .source_freshness
            .key
            .schema_version,
        INDEX_SCHEMA_VERSION + 1
    );
}

#[test]
fn semantic_queries_are_bounded_and_return_pure_dto_results() {
    let document = document(
        "/workspace/src/query.rs",
        1,
        1,
        "pub fn alpha() { beta(); }\npub fn beta() {}\nfn test_alpha() { alpha(); }\n",
    );
    let file_index = LexicalIndexer::new().index_document(
        &document,
        SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
        SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
    );
    let alpha_hash = file_index
        .symbols
        .iter()
        .find(|symbol| symbol.display_name.as_deref() == Some("alpha"))
        .unwrap()
        .symbol_name_hash
        .clone();
    let mut index = SemanticIndex::new();
    assert_eq!(index.upsert(file_index), SemanticUpsertOutcome::Applied);

    let symbols = index.query(&query(SemanticQueryKind::SymbolLookup, None, 1));
    assert_eq!(symbols.status, SemanticQueryStatus::Partial);
    assert_eq!(symbols.results.len(), 1);

    let references = index.query(&query(
        SemanticQueryKind::References,
        Some(alpha_hash.clone()),
        10,
    ));
    assert_eq!(references.status, SemanticQueryStatus::Fresh);
    assert!(
        references
            .results
            .iter()
            .any(|result| result.range.is_some())
    );

    let refactoring = index.query(&query(
        SemanticQueryKind::RefactoringPreview,
        Some(alpha_hash),
        10,
    ));
    assert_eq!(refactoring.status, SemanticQueryStatus::Fresh);
    assert!(refactoring.results.iter().all(|result| {
        result
            .proposal_preview
            .as_ref()
            .is_some_and(|preview| preview.kind == ProposalPayloadKind::WorkspaceEdit)
    }));

    let test_impact = index.query(&query(SemanticQueryKind::TestImpact, None, 10));
    assert!(
        test_impact
            .results
            .iter()
            .any(|result| result.label == "test-impact-source")
    );
}

#[test]
fn retrieval_chunks_two_files_and_returns_metadata_only_citations() {
    let auth = document_with(
        "/workspace/src/auth.rs",
        FileId(41),
        LanguageId("rust".to_string()),
        1,
        1,
        SemanticPrivacyScope::Workspace,
        "pub fn issue_token() {\n    let super_secret_body_marker = \"token-session\";\n    validate_session(super_secret_body_marker);\n}\n",
    );
    let reports = document_with(
        "/workspace/src/reports.rs",
        FileId(42),
        LanguageId("rust".to_string()),
        1,
        1,
        SemanticPrivacyScope::Workspace,
        "pub fn render_report() {\n    let chart_title = \"quarterly summary\";\n    export_pdf(chart_title);\n}\n",
    );
    let indexer = LexicalIndexer::new();
    let mut index = SemanticIndex::new();
    assert_eq!(
        index.upsert(indexer.index_document(
            &auth,
            SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
            SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
        )),
        SemanticUpsertOutcome::Applied
    );
    assert_eq!(
        index.upsert(indexer.index_document(
            &reports,
            SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
            SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
        )),
        SemanticUpsertOutcome::Applied
    );

    let chunks = index.retrieval_chunks();
    assert_eq!(chunks.len(), 2);
    assert!(
        chunks
            .iter()
            .any(|chunk| chunk.citation.path == auth.identity.canonical_path)
    );
    assert!(
        chunks
            .iter()
            .any(|chunk| chunk.citation.path == reports.identity.canonical_path)
    );
    assert!(chunks.iter().all(|chunk| {
        chunk.citation.chunk_fingerprint.algorithm == RETRIEVAL_CHUNK_SHA256_ALGORITHM
            && chunk.citation.chunk_fingerprint.value.len() == 64
            && chunk.citation.freshness.key.content_hash == chunk.citation.chunk_fingerprint
            && chunk.embedding.dimensions == LOCAL_RETRIEVAL_EMBEDDING_DIMENSIONS as u16
            && chunk.embedding.values.len() == LOCAL_RETRIEVAL_EMBEDDING_DIMENSIONS
    }));

    let response = index.search_retrieval(&RetrievalQuery {
        workspace_id: WorkspaceId(7),
        query_text: "session token validation".to_string(),
        file_ids: Vec::new(),
        paths: Vec::new(),
        language_ids: Vec::new(),
        privacy_scope: SemanticPrivacyScope::Workspace,
        freshness_policy: SemanticQueryFreshnessPolicy::RequireFresh,
        limit: 4,
        schema_version: INDEX_SCHEMA_VERSION,
    });

    assert_eq!(response.results.len(), 2);
    let first = response.results.first().unwrap();
    assert_eq!(first.citation.path, auth.identity.canonical_path);
    assert_eq!(first.citation.file_id, FileId(41));
    assert_eq!(first.citation.range.start.line, 0);
    assert!(first.citation.range.end.line >= first.citation.range.start.line);
    assert!(first.citation.byte_range.end > first.citation.byte_range.start);
    assert_eq!(
        first.citation.freshness.key.content_hash,
        first.citation.chunk_fingerprint
    );
    assert_eq!(
        first.freshness.key.content_hash,
        first.citation.chunk_fingerprint
    );

    let rendered = format!("{:?}", first);
    assert!(!rendered.contains("super_secret_body_marker"));
    assert!(!rendered.contains("validate_session(super_secret_body_marker)"));
}

#[test]
fn retrieval_reindex_preserves_unchanged_embeddings_and_updates_changed_sha() {
    let auth_v1 = document_with(
        "/workspace/src/auth.rs",
        FileId(51),
        LanguageId("rust".to_string()),
        1,
        1,
        SemanticPrivacyScope::Workspace,
        "pub fn issue_token() {\n    sign_token(\"alpha\");\n}\n\npub fn audit_login() {\n    record_audit_event(\"stable\");\n}\n",
    );
    let reports_v1 = document_with(
        "/workspace/src/reports.rs",
        FileId(52),
        LanguageId("rust".to_string()),
        1,
        1,
        SemanticPrivacyScope::Workspace,
        "pub fn render_report() {\n    export_pdf(\"stable report\");\n}\n",
    );
    let auth_v2 = document_with(
        "/workspace/src/auth.rs",
        FileId(51),
        LanguageId("rust".to_string()),
        2,
        1,
        SemanticPrivacyScope::Workspace,
        "pub fn issue_token() {\n    sign_token(\"beta\");\n}\n\npub fn audit_login() {\n    record_audit_event(\"stable\");\n}\n",
    );

    let indexer = LexicalIndexer::new();
    let mut index = SemanticIndex::new();
    index.upsert(indexer.index_document(
        &auth_v1,
        SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
        SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
    ));
    index.upsert(indexer.index_document(
        &reports_v1,
        SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
        SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
    ));
    let initial_issue = retrieval_chunk(&index, FileId(51), "issue_token");
    let initial_audit = retrieval_chunk(&index, FileId(51), "audit_login");
    let initial_report = retrieval_chunk(&index, FileId(52), "render_report");

    assert_eq!(
        index.upsert(indexer.index_document(
            &auth_v2,
            SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
            SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
        )),
        SemanticUpsertOutcome::Replaced
    );

    let updated_issue = retrieval_chunk(&index, FileId(51), "issue_token");
    let updated_audit = retrieval_chunk(&index, FileId(51), "audit_login");
    let updated_report = retrieval_chunk(&index, FileId(52), "render_report");

    assert_ne!(
        updated_issue.citation.chunk_fingerprint,
        initial_issue.citation.chunk_fingerprint
    );
    assert_ne!(
        updated_issue.citation.freshness.key.content_hash,
        initial_issue.citation.freshness.key.content_hash
    );
    assert_eq!(
        updated_audit.citation.chunk_fingerprint,
        initial_audit.citation.chunk_fingerprint
    );
    assert_eq!(updated_audit.embedding, initial_audit.embedding);
    assert_eq!(
        updated_report.citation.chunk_fingerprint,
        initial_report.citation.chunk_fingerprint
    );
    assert_eq!(updated_report.embedding, initial_report.embedding);
    assert_eq!(
        updated_report.citation.freshness.key.content_hash,
        initial_report.citation.freshness.key.content_hash
    );
}

fn retrieval_chunk(
    index: &SemanticIndex,
    file_id: FileId,
    label: &str,
) -> devil_index::RetrievalChunkRecord {
    index
        .retrieval_chunks()
        .iter()
        .find(|chunk| chunk.citation.file_id == file_id && chunk.label.contains(label))
        .cloned()
        .expect("retrieval chunk should be indexed")
}

#[test]
fn stale_upserts_are_ignored_and_newer_generations_replace_records() {
    let mut index = SemanticIndex::new();
    let parser = LexicalFallbackParser::new();
    let old_doc = document("/workspace/src/upsert.rs", 2, 2, "fn old_name() {}\n");
    let new_doc = document("/workspace/src/upsert.rs", 3, 3, "fn new_name() {}\n");

    let old_index = parser
        .parse(devil_index::ParseRequest {
            document: old_doc,
            grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
            model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
        })
        .unwrap()
        .file_index;
    let new_index = parser
        .parse(devil_index::ParseRequest {
            document: new_doc,
            grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
            model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
        })
        .unwrap()
        .file_index;

    assert_eq!(index.upsert(new_index), SemanticUpsertOutcome::Applied);
    assert_eq!(index.upsert(old_index), SemanticUpsertOutcome::IgnoredStale);
    assert!(
        index
            .symbols()
            .iter()
            .any(|symbol| symbol.display_name.as_deref() == Some("new_name"))
    );
    assert!(
        !index
            .symbols()
            .iter()
            .any(|symbol| symbol.display_name.as_deref() == Some("old_name"))
    );
}

#[test]
fn semantic_index_replaces_old_generation_when_content_version_resets() {
    let mut index = SemanticIndex::new();
    let parser = LexicalFallbackParser::new();
    let old_doc = document("/workspace/src/reset.rs", 5, 1, "fn old_generation() {}\n");
    let new_doc = document("/workspace/src/reset.rs", 1, 2, "fn new_generation() {}\n");

    let old_index = parser
        .parse(devil_index::ParseRequest {
            document: old_doc,
            grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
            model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
        })
        .unwrap()
        .file_index;
    let new_index = parser
        .parse(devil_index::ParseRequest {
            document: new_doc,
            grammar_version: SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
            model_version: SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
        })
        .unwrap()
        .file_index;

    assert_eq!(
        index.upsert(old_index.clone()),
        SemanticUpsertOutcome::Applied
    );
    assert_eq!(index.upsert(new_index), SemanticUpsertOutcome::Replaced);
    assert_eq!(index.upsert(old_index), SemanticUpsertOutcome::IgnoredStale);
    assert!(
        index
            .symbols()
            .iter()
            .any(|symbol| symbol.display_name.as_deref() == Some("new_generation"))
    );
    assert!(
        !index
            .symbols()
            .iter()
            .any(|symbol| symbol.display_name.as_deref() == Some("old_generation"))
    );
}

#[test]
fn rename_preview_payload_is_proposal_only_and_preserves_preconditions() {
    let document = document(
        "/workspace/src/rename.rs",
        8,
        9,
        "pub fn rename_me() {}\nfn caller() { rename_me(); }\n",
    );
    let file_index = LexicalIndexer::new().index_document(
        &document,
        SemanticGrammarVersion(DEFAULT_GRAMMAR_VERSION.to_string()),
        SemanticModelVersion(DEFAULT_MODEL_VERSION.to_string()),
    );
    let symbol = file_index
        .symbols
        .iter()
        .find(|symbol| symbol.display_name.as_deref() == Some("rename_me"))
        .unwrap();

    let payload = build_rename_preview_payload(symbol, "renamed");
    assert_eq!(payload.workspace_id, WorkspaceId(7));
    assert_eq!(
        payload.source,
        devil_protocol::WorkspaceEditSourceKind::SemanticRefactor
    );
    assert_eq!(payload.required_capability.0, "editor.write");
    assert_eq!(
        payload.target_coverage.coverage_kind,
        ProposalTargetCoverageKind::Complete
    );
    assert_eq!(payload.file_edits.len(), 1);
    assert_eq!(payload.file_edits[0].edits.edits.len(), 2);
    assert_eq!(
        payload.file_edits[0].preconditions.file_content_version,
        Some(FileContentVersion(8))
    );
    assert_eq!(
        payload.file_edits[0].preconditions.workspace_generation,
        Some(WorkspaceGeneration(9))
    );
    assert_eq!(
        payload.file_edits[0].preconditions.expected_fingerprint,
        Some(symbol.invalidation_key.content_hash.clone())
    );
}

#[test]
fn structural_search_matches_metavariables_and_suppression_comments() {
    let document = document(
        "/workspace/src/structural.rs",
        12,
        13,
        "pub fn alpha() {}\n// ast-grep-ignore\npub fn ignored() {}\npub fn beta() {}\n",
    );
    let query = StructuralSearchQuery {
        pattern: "fn $NAME ( )".to_string(),
        rewrite: Some("fn renamed_$NAME ( )".to_string()),
        result_limit: 10,
    };

    let report = run_structural_search(&document, &query);

    assert_eq!(report.matches.len(), 2);
    assert_eq!(report.omitted_match_count, 0);
    assert_eq!(report.matches[0].capture_value("NAME"), Some("alpha"));
    assert_eq!(
        report.matches[0].replacement_preview.as_deref(),
        Some("fn renamed_alpha ( )")
    );
    assert_eq!(report.matches[1].capture_value("NAME"), Some("beta"));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "structural_search.suppressed")
    );
}

#[test]
fn structural_rewrite_payload_is_cross_file_proposal_only() {
    let first = document_with(
        "/workspace/src/one.rs",
        FileId(11),
        LanguageId("rust".to_string()),
        21,
        22,
        SemanticPrivacyScope::Workspace,
        "pub fn alpha() {}\n",
    );
    let second = document_with(
        "/workspace/src/two.rs",
        FileId(12),
        LanguageId("rust".to_string()),
        23,
        22,
        SemanticPrivacyScope::Workspace,
        "pub fn beta() {}\n",
    );
    let query = StructuralSearchQuery {
        pattern: "fn $NAME ( )".to_string(),
        rewrite: Some("fn renamed_$NAME ( )".to_string()),
        result_limit: 10,
    };
    let first_report = run_structural_search(&first, &query);
    let second_report = run_structural_search(&second, &query);

    let payload = build_structural_rewrite_preview_payload(
        WorkspaceId(7),
        "structural replace fn names",
        &[
            StructuralRewriteFileInput {
                document: &first,
                matches: &first_report.matches,
            },
            StructuralRewriteFileInput {
                document: &second,
                matches: &second_report.matches,
            },
        ],
    );

    assert_eq!(payload.workspace_id, WorkspaceId(7));
    assert_eq!(
        payload.source,
        WorkspaceEditSourceKind::StructuralSearchReplace
    );
    assert_eq!(payload.required_capability.0, "editor.write");
    assert_eq!(
        payload.target_coverage.coverage_kind,
        ProposalTargetCoverageKind::Complete
    );
    assert_eq!(payload.target_coverage.targets.len(), 2);
    assert_eq!(payload.file_edits.len(), 2);
    assert_eq!(
        payload.file_edits[0].edits.edits[0].replacement,
        "fn renamed_alpha ( )"
    );
    assert_eq!(
        payload.file_edits[1].edits.edits[0].replacement,
        "fn renamed_beta ( )"
    );
    assert_eq!(
        payload.file_edits[0].preconditions.file_content_version,
        Some(FileContentVersion(21))
    );
    assert_eq!(
        payload.file_edits[1].preconditions.file_content_version,
        Some(FileContentVersion(23))
    );
    assert_eq!(
        payload.file_edits[0].preconditions.expected_fingerprint,
        Some(first.identity.content_hash.clone())
    );
    assert!(payload.diagnostics.is_empty());
}
