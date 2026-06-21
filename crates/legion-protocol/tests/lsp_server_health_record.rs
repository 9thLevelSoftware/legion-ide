use legion_protocol::{
    CapabilityDecisionId, LanguageId, LanguageServerId, LspResultStatus,
    LspServerBinaryProvenance, LspServerHealthRecord,
};

#[test]
fn health_record_round_trips_metadata_only() {
    let record = LspServerHealthRecord {
        server_id: LanguageServerId(1),
        language_id: LanguageId("rust".to_string()),
        binary_provenance: LspServerBinaryProvenance::SystemPath,
        binary_path_hash: None,
        artifact_hash: None,
        version: Some("rust-analyzer 1.0.0".into()),
        init_status: LspResultStatus::Fresh,
        capabilities: Vec::new(),
        diagnostics_latency_ms: Some(42),
        restart_count: 0,
        download_decision_id: None,
        schema_version: LspServerHealthRecord::schema_version(),
    };
    let json = serde_json::to_string(&record).unwrap();
    // Metadata only: no raw source / payload fields leak in.
    assert!(!json.contains("source_text"));
    let back: LspServerHealthRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(back.version.as_deref(), Some("rust-analyzer 1.0.0"));
    assert_eq!(back.restart_count, 0);
}

#[test]
fn downloaded_provenance_carries_decision_id() {
    let record = LspServerHealthRecord {
        server_id: LanguageServerId(2),
        language_id: LanguageId("rust".to_string()),
        binary_provenance: LspServerBinaryProvenance::Downloaded,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: LspResultStatus::Unavailable,
        capabilities: Vec::new(),
        diagnostics_latency_ms: None,
        restart_count: 2,
        download_decision_id: Some(CapabilityDecisionId(7)),
        schema_version: LspServerHealthRecord::schema_version(),
    };
    assert_eq!(record.download_decision_id, Some(CapabilityDecisionId(7)));
}
