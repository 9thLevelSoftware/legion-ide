use legion_ai::{
    assemble_context_manifest, assemble_context_manifest_from_sources, collect_file_context,
    ManifestMetadata,
};
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

// ---------------------------------------------------------------------------
// P4.F2.T3 — egress-equality across all 7 source categories and assembly paths
// ---------------------------------------------------------------------------

fn default_metadata_t3() -> ManifestMetadata {
    ManifestMetadata {
        workspace_id: Some(WorkspaceId(10)),
        proposal_id: None,
        purpose: ContextManifestPurpose::Explanation,
        workspace_trust_state: None,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        permissions: Vec::new(),
        generated_at: TimestampMillis(300),
        schema_version: 1,
    }
}

/// All 7 source categories — File, UserSelection, SemanticRecord,
/// LspDiagnosticSummary, TerminalSummary, MemoryRecord, Rule — must survive
/// the egress path unchanged when Included.
#[test]
fn egress_equality_all_seven_source_categories() {
    let assembly = ContextManifestAssembly {
        manifest_id: "egress:7cat".to_string(),
        workspace_id: Some(WorkspaceId(10)),
        proposal_id: None,
        purpose: ContextManifestPurpose::Explanation,
        workspace_trust_state: None,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        sources: ContextManifestSources {
            files: vec![manifest_item(
                "file:cat",
                ContextManifestItemKind::File,
                ContextManifestInclusionState::Included,
            )],
            selections: vec![manifest_item(
                "sel:cat",
                ContextManifestItemKind::UserSelection,
                ContextManifestInclusionState::Included,
            )],
            symbols: vec![manifest_item(
                "sym:cat",
                ContextManifestItemKind::SemanticRecord,
                ContextManifestInclusionState::Included,
            )],
            diagnostics: vec![manifest_item(
                "diag:cat",
                ContextManifestItemKind::LspDiagnosticSummary,
                ContextManifestInclusionState::Included,
            )],
            terminal_excerpts: vec![manifest_item(
                "term:cat",
                ContextManifestItemKind::TerminalSummary,
                ContextManifestInclusionState::Included,
            )],
            memory: vec![manifest_item(
                "mem:cat",
                ContextManifestItemKind::MemoryRecord,
                ContextManifestInclusionState::Included,
            )],
            rules: vec![manifest_item(
                "rule:cat",
                ContextManifestItemKind::Rule,
                ContextManifestInclusionState::Included,
            )],
        },
        permissions: Vec::new(),
        omitted_item_count: 0,
        stale_or_missing_metadata_risk_present: false,
        generated_at: TimestampMillis(300),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let record = assemble_context_manifest(assembly);
    assert_eq!(record.items.len(), 7, "all 7 source categories must appear");

    let egress = redacted_for_egress(&record);
    assert_eq!(
        egress.items.len(),
        7,
        "all 7 Included items must survive egress filtering"
    );

    let expected_kinds = [
        ContextManifestItemKind::File,
        ContextManifestItemKind::UserSelection,
        ContextManifestItemKind::SemanticRecord,
        ContextManifestItemKind::LspDiagnosticSummary,
        ContextManifestItemKind::TerminalSummary,
        ContextManifestItemKind::MemoryRecord,
        ContextManifestItemKind::Rule,
    ];
    for kind in &expected_kinds {
        assert!(
            egress.items.iter().any(|i| i.kind == *kind),
            "egress must contain a {:?} item",
            kind
        );
    }
}

/// Items in states other than Included (Excluded, Redacted, Denied, Omitted)
/// must be stripped from the egress record.
#[test]
fn egress_equality_with_mixed_inclusion_states() {
    let assembly = ContextManifestAssembly {
        manifest_id: "egress:mixed".to_string(),
        workspace_id: Some(WorkspaceId(10)),
        proposal_id: None,
        purpose: ContextManifestPurpose::Explanation,
        workspace_trust_state: None,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        sources: ContextManifestSources {
            files: vec![
                manifest_item(
                    "file:inc",
                    ContextManifestItemKind::File,
                    ContextManifestInclusionState::Included,
                ),
                manifest_item(
                    "file:exc",
                    ContextManifestItemKind::File,
                    ContextManifestInclusionState::Excluded,
                ),
                manifest_item(
                    "file:red",
                    ContextManifestItemKind::File,
                    ContextManifestInclusionState::Redacted,
                ),
                manifest_item(
                    "file:den",
                    ContextManifestItemKind::File,
                    ContextManifestInclusionState::Denied,
                ),
                manifest_item(
                    "file:omi",
                    ContextManifestItemKind::File,
                    ContextManifestInclusionState::Omitted,
                ),
            ],
            selections: Vec::new(),
            symbols: Vec::new(),
            diagnostics: Vec::new(),
            terminal_excerpts: Vec::new(),
            memory: Vec::new(),
            rules: Vec::new(),
        },
        permissions: Vec::new(),
        omitted_item_count: 4,
        stale_or_missing_metadata_risk_present: false,
        generated_at: TimestampMillis(300),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let record = assemble_context_manifest(assembly);
    assert_eq!(record.items.len(), 5, "all 5 items must be present before egress filtering");

    let egress = redacted_for_egress(&record);
    assert_eq!(
        egress.items.len(),
        1,
        "only the Included item must survive egress filtering"
    );
    assert_eq!(egress.items[0].item_id, "file:inc");
    assert_eq!(
        egress.items[0].inclusion,
        ContextManifestInclusionState::Included
    );
}

/// Excluded items must never appear in the serialised egress bytes — not even
/// as metadata fragments.
#[test]
fn excluded_items_never_appear_in_egress_bytes() {
    let assembly = ContextManifestAssembly {
        manifest_id: "egress:no-excluded".to_string(),
        workspace_id: None,
        proposal_id: None,
        purpose: ContextManifestPurpose::Explanation,
        workspace_trust_state: None,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        sources: ContextManifestSources {
            files: vec![
                manifest_item(
                    "visible-file",
                    ContextManifestItemKind::File,
                    ContextManifestInclusionState::Included,
                ),
                manifest_item(
                    "secret-file",
                    ContextManifestItemKind::File,
                    ContextManifestInclusionState::Excluded,
                ),
            ],
            selections: vec![manifest_item(
                "secret-selection",
                ContextManifestItemKind::UserSelection,
                ContextManifestInclusionState::Excluded,
            )],
            symbols: vec![manifest_item(
                "secret-symbol",
                ContextManifestItemKind::SemanticRecord,
                ContextManifestInclusionState::Excluded,
            )],
            diagnostics: Vec::new(),
            terminal_excerpts: Vec::new(),
            memory: vec![manifest_item(
                "secret-memory",
                ContextManifestItemKind::MemoryRecord,
                ContextManifestInclusionState::Excluded,
            )],
            rules: Vec::new(),
        },
        permissions: Vec::new(),
        omitted_item_count: 4,
        stale_or_missing_metadata_risk_present: false,
        generated_at: TimestampMillis(300),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let record = assemble_context_manifest(assembly);
    let egress = redacted_for_egress(&record);

    let egress_text =
        String::from_utf8(serde_json::to_vec(&egress).expect("serialize egress"))
            .expect("egress bytes are valid UTF-8");

    assert!(
        egress_text.contains("visible-file"),
        "included item must be present in egress bytes"
    );
    assert!(
        !egress_text.contains("secret-file"),
        "excluded file must not appear in egress bytes"
    );
    assert!(
        !egress_text.contains("secret-selection"),
        "excluded selection must not appear in egress bytes"
    );
    assert!(
        !egress_text.contains("secret-symbol"),
        "excluded symbol must not appear in egress bytes"
    );
    assert!(
        !egress_text.contains("secret-memory"),
        "excluded memory item must not appear in egress bytes"
    );
}

/// Given identical source items, both assembly paths must produce the same
/// egress-filtered item set (same item_ids, kinds, and inclusion states in
/// the same order after serialisation).
#[test]
fn egress_is_deterministic_across_assembly_paths() {
    let paths = vec![
        CanonicalPath("C:/repo/alpha.rs".to_string()),
        CanonicalPath("C:/repo/beta.rs".to_string()),
    ];
    // Build items via the collector (the from_sources path uses these internally).
    let items = collect_file_context(&paths, WorkspaceId(10));

    // Path A: assemble_context_manifest with the same items in sources.
    let assembly_a = ContextManifestAssembly {
        manifest_id: "egress:det:path-a".to_string(),
        workspace_id: Some(WorkspaceId(10)),
        proposal_id: None,
        purpose: ContextManifestPurpose::Explanation,
        workspace_trust_state: None,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        sources: ContextManifestSources {
            files: items.clone(),
            selections: Vec::new(),
            symbols: Vec::new(),
            diagnostics: Vec::new(),
            terminal_excerpts: Vec::new(),
            memory: Vec::new(),
            rules: Vec::new(),
        },
        permissions: Vec::new(),
        omitted_item_count: 0,
        stale_or_missing_metadata_risk_present: true,
        generated_at: TimestampMillis(300),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let record_a = assemble_context_manifest(assembly_a);

    // Path B: assemble_context_manifest_from_sources with identical source items.
    let sources_b = ContextManifestSources {
        files: items,
        selections: Vec::new(),
        symbols: Vec::new(),
        diagnostics: Vec::new(),
        terminal_excerpts: Vec::new(),
        memory: Vec::new(),
        rules: Vec::new(),
    };
    let record_b = assemble_context_manifest_from_sources(sources_b, default_metadata_t3());

    // Egress item sets must be identical regardless of assembly path used.
    let egress_items_a = redacted_for_egress(&record_a).items;
    let egress_items_b = redacted_for_egress(&record_b).items;

    let bytes_a = serde_json::to_vec(&egress_items_a).expect("serialize path A items");
    let bytes_b = serde_json::to_vec(&egress_items_b).expect("serialize path B items");

    assert_eq!(
        bytes_a, bytes_b,
        "egress item bytes must be identical across both assembly paths"
    );
}
