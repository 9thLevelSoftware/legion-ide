use legion_ai::{
    assemble_context_manifest, assemble_context_manifest_from_sources, collect_file_context,
    ManifestMetadata,
};
use legion_protocol::*;

fn manifest_item(item_id: &str, kind: ContextManifestItemKind) -> ContextManifestItem {
    ContextManifestItem {
        item_id: item_id.to_string(),
        kind,
        inclusion: ContextManifestInclusionState::Included,
        workspace_id: None,
        file_id: None,
        buffer_id: None,
        proposal_id: None,
        target_id: None,
        path: None,
        ranges: Vec::new(),
        counts: Vec::new(),
        hashes: Vec::new(),
        privacy_scope: None,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        freshness: None,
        preconditions: None,
        labels: Vec::new(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

#[test]
fn assemble_context_manifest_preserves_item_inclusion_states() {
    let assembly = ContextManifestAssembly {
        manifest_id: "ai:context:manifest:2".to_string(),
        workspace_id: None,
        proposal_id: None,
        purpose: ContextManifestPurpose::Explanation,
        workspace_trust_state: None,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        sources: ContextManifestSources {
            files: vec![manifest_item("file-2", ContextManifestItemKind::File)],
            selections: Vec::new(),
            symbols: vec![ContextManifestItem {
                inclusion: ContextManifestInclusionState::Excluded,
                ..manifest_item("symbol-2", ContextManifestItemKind::SemanticRecord)
            }],
            diagnostics: Vec::new(),
            terminal_excerpts: Vec::new(),
            memory: Vec::new(),
            rules: Vec::new(),
        },
        permissions: Vec::new(),
        omitted_item_count: 0,
        stale_or_missing_metadata_risk_present: false,
        generated_at: TimestampMillis(8),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let record = assemble_context_manifest(assembly);
    assert_eq!(record.items.len(), 2);
    assert_eq!(
        record.items[1].inclusion,
        ContextManifestInclusionState::Excluded
    );
}

#[test]
fn assemble_context_manifest_returns_structured_record() {
    let assembly = ContextManifestAssembly {
        manifest_id: "ai:context:manifest:1".to_string(),
        workspace_id: None,
        proposal_id: None,
        purpose: ContextManifestPurpose::Explanation,
        workspace_trust_state: None,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        sources: ContextManifestSources {
            files: vec![manifest_item("file-1", ContextManifestItemKind::File)],
            selections: vec![manifest_item(
                "selection-1",
                ContextManifestItemKind::UserSelection,
            )],
            symbols: vec![manifest_item(
                "symbol-1",
                ContextManifestItemKind::SemanticRecord,
            )],
            diagnostics: vec![manifest_item(
                "diagnostic-1",
                ContextManifestItemKind::LspDiagnosticSummary,
            )],
            terminal_excerpts: vec![manifest_item(
                "terminal-1",
                ContextManifestItemKind::TerminalSummary,
            )],
            memory: vec![manifest_item(
                "memory-1",
                ContextManifestItemKind::MemoryRecord,
            )],
            rules: vec![manifest_item("rule-1", ContextManifestItemKind::Rule)],
        },
        permissions: Vec::new(),
        omitted_item_count: 0,
        stale_or_missing_metadata_risk_present: false,
        generated_at: TimestampMillis(7),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let record = assemble_context_manifest(assembly);
    assert_eq!(record.manifest_id, "ai:context:manifest:1");
    assert_eq!(record.items.len(), 7);
    assert!(matches!(
        record.items[0].kind,
        ContextManifestItemKind::File
    ));
    assert!(matches!(
        record.items[6].kind,
        ContextManifestItemKind::Rule
    ));
    assert_eq!(record.generated_at, TimestampMillis(7));
    assert_eq!(record.redaction_hints, vec![RedactionHint::MetadataOnly]);
}

// ---------------------------------------------------------------------------
// P4.F2.T1 — assemble_context_manifest_from_sources tests
// ---------------------------------------------------------------------------

fn default_metadata() -> ManifestMetadata {
    ManifestMetadata {
        workspace_id: None,
        proposal_id: None,
        purpose: ContextManifestPurpose::Explanation,
        workspace_trust_state: None,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        permissions: Vec::new(),
        generated_at: TimestampMillis(200),
        schema_version: 1,
    }
}

fn excluded_manifest_item(item_id: &str, kind: ContextManifestItemKind) -> ContextManifestItem {
    ContextManifestItem {
        item_id: item_id.to_string(),
        kind,
        inclusion: ContextManifestInclusionState::Excluded,
        workspace_id: None,
        file_id: None,
        buffer_id: None,
        proposal_id: None,
        target_id: None,
        path: None,
        ranges: Vec::new(),
        counts: Vec::new(),
        hashes: Vec::new(),
        privacy_scope: None,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        freshness: None,
        preconditions: None,
        labels: Vec::new(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn empty_sources() -> ContextManifestSources {
    ContextManifestSources {
        files: Vec::new(),
        selections: Vec::new(),
        symbols: Vec::new(),
        diagnostics: Vec::new(),
        terminal_excerpts: Vec::new(),
        memory: Vec::new(),
        rules: Vec::new(),
    }
}

#[test]
fn assemble_from_file_sources_produces_file_items() {
    let paths = vec![
        CanonicalPath("C:/repo/src/main.rs".to_string()),
        CanonicalPath("C:/repo/src/lib.rs".to_string()),
    ];
    let items = collect_file_context(&paths, WorkspaceId(1));

    assert_eq!(items.len(), 2);
    for item in &items {
        assert!(
            matches!(item.kind, ContextManifestItemKind::File),
            "expected File kind, got {:?}",
            item.kind
        );
        assert_eq!(
            item.inclusion,
            ContextManifestInclusionState::Included,
            "collector should default to Included"
        );
        assert_eq!(
            item.workspace_id,
            Some(WorkspaceId(1)),
            "workspace_id should be propagated"
        );
        assert_eq!(
            item.redaction_hints,
            vec![RedactionHint::MetadataOnly],
            "metadata-only redaction required"
        );
    }
    assert_eq!(
        items[0].path,
        Some(CanonicalPath("C:/repo/src/main.rs".to_string()))
    );
    assert_eq!(
        items[1].path,
        Some(CanonicalPath("C:/repo/src/lib.rs".to_string()))
    );
}

#[test]
fn assemble_computes_omitted_count_from_excluded_items() {
    let sources = ContextManifestSources {
        files: vec![
            manifest_item("file-included", ContextManifestItemKind::File),
            excluded_manifest_item("file-excluded", ContextManifestItemKind::File),
        ],
        selections: vec![excluded_manifest_item(
            "sel-excluded",
            ContextManifestItemKind::UserSelection,
        )],
        ..empty_sources()
    };

    let record = assemble_context_manifest_from_sources(sources, default_metadata());

    assert_eq!(
        record.omitted_item_count, 2,
        "two excluded items should yield omitted_item_count=2"
    );
    assert_eq!(
        record.items.len(),
        3,
        "all items (included and excluded) must be present in the record"
    );
}

#[test]
fn assemble_detects_stale_metadata_risk() {
    // collect_file_context produces items with freshness: None,
    // which represents missing freshness metadata — a stale-risk signal.
    let paths = vec![CanonicalPath("C:/repo/main.rs".to_string())];
    let items = collect_file_context(&paths, WorkspaceId(2));

    let sources = ContextManifestSources {
        files: items,
        ..empty_sources()
    };

    let record = assemble_context_manifest_from_sources(sources, default_metadata());

    assert!(
        record.stale_or_missing_metadata_risk_present,
        "items with freshness: None should set stale_or_missing_metadata_risk_present"
    );
}

#[test]
fn manifest_id_is_deterministic_for_same_inputs() {
    let paths = vec![
        CanonicalPath("C:/repo/src/a.rs".to_string()),
        CanonicalPath("C:/repo/src/b.rs".to_string()),
    ];
    let items1 = collect_file_context(&paths, WorkspaceId(3));
    let items2 = collect_file_context(&paths, WorkspaceId(3));

    let sources1 = ContextManifestSources {
        files: items1,
        ..empty_sources()
    };
    let sources2 = ContextManifestSources {
        files: items2,
        ..empty_sources()
    };

    let meta = default_metadata();
    let record1 = assemble_context_manifest_from_sources(sources1, meta.clone());
    let record2 = assemble_context_manifest_from_sources(sources2, meta);

    assert_eq!(
        record1.manifest_id, record2.manifest_id,
        "identical inputs must produce the same manifest_id"
    );
}

#[test]
fn empty_sources_produce_empty_manifest() {
    let sources = empty_sources();
    let record = assemble_context_manifest_from_sources(sources, default_metadata());

    assert!(
        record.items.is_empty(),
        "empty sources must produce a manifest with no items"
    );
    assert_eq!(
        record.omitted_item_count, 0,
        "empty sources must have omitted_item_count=0"
    );
    assert!(
        !record.stale_or_missing_metadata_risk_present,
        "empty sources must not flag stale metadata risk"
    );
}
