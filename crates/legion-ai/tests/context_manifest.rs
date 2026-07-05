use legion_ai::assemble_context_manifest;
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
