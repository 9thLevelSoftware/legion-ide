use legion_ai::assemble_context_manifest;
use legion_protocol::*;

fn manifest_item(
    item_id: &str,
    kind: ContextManifestItemKind,
    inclusion: ContextManifestInclusionState,
) -> ContextManifestItem {
    ContextManifestItem {
        item_id: item_id.to_string(),
        kind,
        inclusion,
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

fn assembled_manifest() -> ContextManifestAssembly {
    ContextManifestAssembly {
        manifest_id: "ai:context:manifest:egress".to_string(),
        workspace_id: Some(WorkspaceId(42)),
        proposal_id: Some(ProposalId(7)),
        purpose: ContextManifestPurpose::ProposalReview,
        workspace_trust_state: Some(WorkspaceTrustState::Trusted),
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        sources: ContextManifestSources {
            files: vec![
                manifest_item(
                    "file:included",
                    ContextManifestItemKind::File,
                    ContextManifestInclusionState::Included,
                ),
                manifest_item(
                    "file:redacted",
                    ContextManifestItemKind::File,
                    ContextManifestInclusionState::Excluded,
                ),
            ],
            selections: vec![manifest_item(
                "selection:included",
                ContextManifestItemKind::UserSelection,
                ContextManifestInclusionState::Included,
            )],
            symbols: Vec::new(),
            diagnostics: Vec::new(),
            terminal_excerpts: Vec::new(),
            memory: Vec::new(),
            rules: vec![manifest_item(
                "rule:included",
                ContextManifestItemKind::Rule,
                ContextManifestInclusionState::Included,
            )],
        },
        permissions: vec![ContextManifestPermissionSummary {
            kind: ContextManifestPermissionKind::ModelProvider,
            capability: CapabilityId("ai.provider.invoke".to_string()),
            principal: Some(PrincipalId("principal:egress".to_string())),
            decision_id: None,
            granted: true,
            privacy_scope: SemanticPrivacyScope::MetadataOnly,
            egress: ContextManifestEgressStatus::LocalOnly,
            risk_label: ProposalRiskLabel::Low,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        omitted_item_count: 1,
        stale_or_missing_metadata_risk_present: false,
        generated_at: TimestampMillis(4242),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn redacted_for_egress(record: &ContextManifestRecord) -> ContextManifestRecord {
    let mut redacted = record.clone();
    redacted
        .items
        .retain(|item| item.inclusion == ContextManifestInclusionState::Included);
    redacted
}

#[test]
fn egress_bytes_match_redacted_manifest_bytes() {
    let manifest = assemble_context_manifest(assembled_manifest());
    let expected_egress = ContextManifestRecord {
        items: vec![
            manifest_item(
                "file:included",
                ContextManifestItemKind::File,
                ContextManifestInclusionState::Included,
            ),
            manifest_item(
                "selection:included",
                ContextManifestItemKind::UserSelection,
                ContextManifestInclusionState::Included,
            ),
            manifest_item(
                "rule:included",
                ContextManifestItemKind::Rule,
                ContextManifestInclusionState::Included,
            ),
        ],
        ..manifest.clone()
    };

    let actual_bytes =
        serde_json::to_vec(&redacted_for_egress(&manifest)).expect("serialize redacted manifest");
    let expected_bytes = serde_json::to_vec(&expected_egress).expect("serialize expected egress");

    assert_eq!(actual_bytes, expected_bytes);

    let actual_text = String::from_utf8(actual_bytes).expect("manifest bytes are utf-8");
    assert!(actual_text.contains("file:included"));
    assert!(actual_text.contains("selection:included"));
    assert!(actual_text.contains("rule:included"));
    assert!(!actual_text.contains("file:redacted"));
}
