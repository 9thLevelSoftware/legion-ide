use std::fs;
use std::path::Path;

use devil_index::{
    DEFAULT_GRAMMAR_VERSION, DEFAULT_MODEL_VERSION, INDEX_SCHEMA_VERSION, IndexError,
    IndexWorkItem, IndexWorkKind, IndexingActor, LexicalFallbackParser, LexicalIndexer,
    ParserWorker, RepositoryScanConfig, RepositoryScanner, SemanticIndex, SemanticUpsertOutcome,
    SourceDocument, SyntaxTreeCache, WorkCompletionState, WorkPriority,
    build_rename_preview_payload, semantic_cancellation_token,
};
use devil_protocol::{
    CancellationTokenId, CanonicalPath, CausalityId, CorrelationId, FileContentVersion,
    FileFingerprint, FileId, LanguageId, ProposalPayloadKind, ProposalTargetCoverageKind,
    SemanticCancellationReason, SemanticGrammarVersion, SemanticGraphRecordKind,
    SemanticModelVersion, SemanticPrivacyScope, SemanticQueryFreshnessPolicy, SemanticQueryId,
    SemanticQueryKind, SemanticQueryRequest, SemanticQueryScope, SemanticQueryStatus, SnapshotId,
    WorkspaceGeneration, WorkspaceId,
};
use uuid::Uuid;

fn document(path: &str, version: u64, generation: u64, text: &str) -> SourceDocument {
    SourceDocument::with_versions(
        WorkspaceId(7),
        FileId(11),
        CanonicalPath(path.to_string()),
        LanguageId("rust".to_string()),
        FileContentVersion(version),
        WorkspaceGeneration(generation),
        Some(SnapshotId(100 + u128::from(version))),
        SemanticPrivacyScope::Workspace,
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
fn repository_scanner_honors_ignore_rules_bounds_and_deterministic_fingerprints() {
    let root = temp_root("scan");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("target")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn visible() {}\n").unwrap();
    fs::write(root.join("target/ignored.rs"), "pub fn ignored() {}\n").unwrap();
    fs::write(root.join("skip.log"), "ignored\n").unwrap();
    fs::write(root.join(".gitignore"), "generated\n").unwrap();
    fs::create_dir_all(root.join("generated")).unwrap();
    fs::write(root.join("generated/ignored.rs"), "pub fn generated() {}\n").unwrap();

    let mut config = RepositoryScanConfig::new(&root, WorkspaceId(55));
    config.max_files = 1;
    config.max_depth = 8;
    let output = RepositoryScanner::new().scan(&config).unwrap();

    assert_eq!(output.files.len(), 1);
    assert_eq!(output.files[0].relative_path, "src/lib.rs");
    assert_eq!(output.files[0].language_id, LanguageId("rust".to_string()));
    assert_eq!(
        output.files[0].identity.content_hash.algorithm,
        "fnv1a64-devil-index-v1"
    );
    assert!(
        output
            .ignored_paths
            .iter()
            .any(|path| path == "target" || path == "target/ignored.rs")
    );
    assert!(output.ignored_paths.iter().any(|path| path == "generated"));
    assert!(output.ignored_paths.iter().any(|path| path == "skip.log"));

    fs::remove_dir_all(root).unwrap();
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
fn syntax_cache_keys_by_content_hash_language_and_grammar_version() {
    let document = document("/workspace/src/cache.rs", 1, 1, "fn cached() {}\n");
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
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.invalidate_grammar(&grammar_one), 1);
    assert_eq!(cache.len(), 1);
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

fn temp_root(label: &str) -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("devil-index-{label}-{unique}"));
    if Path::new(&root).exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    fs::create_dir_all(&root).unwrap();
    root
}
