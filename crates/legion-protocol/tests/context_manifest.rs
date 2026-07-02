use legion_protocol::*;

fn manifest_item(item_id: &str, kind: ContextManifestItemKind) -> ContextManifestItem {
    ContextManifestItem {
        item_id: item_id.to_string(),
        kind,
        inclusion: ContextManifestInclusionState::Included,
        workspace_id: Some(WorkspaceId(11)),
        file_id: Some(FileId(22)),
        buffer_id: Some(BufferId(33)),
        proposal_id: Some(ProposalId(44)),
        target_id: Some(format!("target:{item_id}")),
        path: Some(CanonicalPath(format!("/repo/{item_id}.rs"))),
        ranges: vec![ByteRange::new(5, 9)],
        counts: vec![ContextManifestItemCount {
            label: "count".to_string(),
            count: 1,
        }],
        hashes: vec![FileFingerprint {
            algorithm: "sha256".to_string(),
            value: format!("hash:{item_id}"),
        }],
        privacy_scope: Some(SemanticPrivacyScope::Workspace),
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        freshness: None,
        preconditions: None,
        labels: vec![format!("label:{item_id}")],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn permission_summary() -> ContextManifestPermissionSummary {
    ContextManifestPermissionSummary {
        kind: ContextManifestPermissionKind::Tool,
        capability: CapabilityId("tool.plan".to_string()),
        principal: Some(PrincipalId("principal-1".to_string())),
        decision_id: Some(CapabilityDecisionId(7)),
        granted: true,
        privacy_scope: SemanticPrivacyScope::Workspace,
        egress: ContextManifestEgressStatus::LocalOnly,
        risk_label: ProposalRiskLabel::Low,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

#[test]
fn context_manifest_assembly_flattens_metadata_only_sources() {
    let assembly = ContextManifestAssembly {
        manifest_id: "ctx:manifest:1".to_string(),
        workspace_id: Some(WorkspaceId(11)),
        proposal_id: Some(ProposalId(44)),
        purpose: ContextManifestPurpose::ProposalReview,
        workspace_trust_state: Some(WorkspaceTrustState::Trusted),
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
        permissions: vec![permission_summary()],
        omitted_item_count: 0,
        stale_or_missing_metadata_risk_present: false,
        generated_at: TimestampMillis(99),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let record = assembly.into_record();
    assert_eq!(record.manifest_id, "ctx:manifest:1");
    assert_eq!(record.items.len(), 7);
    assert!(matches!(
        record.items[0].kind,
        ContextManifestItemKind::File
    ));
    assert!(matches!(
        record.items[1].kind,
        ContextManifestItemKind::UserSelection
    ));
    assert!(matches!(
        record.items[2].kind,
        ContextManifestItemKind::SemanticRecord
    ));
    assert!(matches!(
        record.items[3].kind,
        ContextManifestItemKind::LspDiagnosticSummary
    ));
    assert!(matches!(
        record.items[4].kind,
        ContextManifestItemKind::TerminalSummary
    ));
    assert!(matches!(
        record.items[5].kind,
        ContextManifestItemKind::MemoryRecord
    ));
    assert!(matches!(
        record.items[6].kind,
        ContextManifestItemKind::Rule
    ));
    assert_eq!(record.permissions.len(), 1);
    assert_eq!(record.redaction_hints, vec![RedactionHint::MetadataOnly]);

    let json = serde_json::to_value(&record).unwrap();
    let json_text = json.to_string();
    assert!(json_text.contains("ctx:manifest:1"));
    assert!(json_text.contains("rule-1"));
    assert!(
        !json_text.contains("freeform"),
        "manifest JSON must remain structured DTO data"
    );
}
