use legion_protocol::{
    LanguageId, LanguageServerId, LspResultStatus, LspServerBinaryProvenance,
    LspServerHealthRecord,
};
use legion_ui::project_lsp_health;

#[test]
fn health_projection_labels_provenance_and_status() {
    let record = LspServerHealthRecord {
        server_id: LanguageServerId(1),
        language_id: LanguageId("rust".into()),
        binary_provenance: LspServerBinaryProvenance::SystemPath,
        binary_path_hash: None,
        artifact_hash: None,
        version: Some("rust-analyzer 1.0.0".into()),
        init_status: LspResultStatus::Fresh,
        capabilities: Vec::new(),
        diagnostics_latency_ms: Some(12),
        restart_count: 1,
        download_decision_id: None,
        schema_version: LspServerHealthRecord::schema_version(),
    };
    let p = project_lsp_health(&record, false);
    assert!(
        p.provenance_label.to_lowercase().contains("path"),
        "provenance_label '{}' should contain 'path' for SystemPath",
        p.provenance_label
    );
    assert!(
        p.version_label.contains("1.0.0"),
        "version_label '{}' should contain '1.0.0'",
        p.version_label
    );
    assert_eq!(p.restart_count, 1);
    assert!(!p.download_refused);
}

#[test]
fn health_projection_all_status_variants_covered() {
    let base = LspServerHealthRecord {
        server_id: LanguageServerId(2),
        language_id: LanguageId("rust".into()),
        binary_provenance: LspServerBinaryProvenance::Bundled,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: LspResultStatus::Fresh,
        capabilities: Vec::new(),
        diagnostics_latency_ms: None,
        restart_count: 0,
        download_decision_id: None,
        schema_version: LspServerHealthRecord::schema_version(),
    };

    let cases = [
        (LspResultStatus::Fresh, "ready"),
        (LspResultStatus::Stale, "stale"),
        (LspResultStatus::Partial, "partial"),
        (LspResultStatus::Cancelled, "cancelled"),
        (LspResultStatus::Timeout, "timed out"),
        (LspResultStatus::Unavailable, "unavailable"),
        (LspResultStatus::Degraded, "degraded"),
    ];
    for (status, expected_label) in cases {
        let mut record = base.clone();
        record.init_status = status;
        let p = project_lsp_health(&record, false);
        assert_eq!(
            p.status_label, expected_label,
            "status {:?} should map to label '{}'",
            status, expected_label
        );
    }
}

#[test]
fn health_projection_download_refused_flag_is_passed_through() {
    let record = LspServerHealthRecord {
        server_id: LanguageServerId(3),
        language_id: LanguageId("rust".into()),
        binary_provenance: LspServerBinaryProvenance::Downloaded,
        binary_path_hash: None,
        artifact_hash: None,
        version: Some("1.2.3".into()),
        init_status: LspResultStatus::Unavailable,
        capabilities: Vec::new(),
        diagnostics_latency_ms: None,
        restart_count: 0,
        download_decision_id: None,
        schema_version: LspServerHealthRecord::schema_version(),
    };
    let refused = project_lsp_health(&record, true);
    assert!(refused.download_refused);
    assert_eq!(refused.provenance_label, "downloaded");

    let allowed = project_lsp_health(&record, false);
    assert!(!allowed.download_refused);
}

#[test]
fn health_projection_version_unknown_when_none() {
    let record = LspServerHealthRecord {
        server_id: LanguageServerId(4),
        language_id: LanguageId("rust".into()),
        binary_provenance: LspServerBinaryProvenance::Configured,
        binary_path_hash: None,
        artifact_hash: None,
        version: None,
        init_status: LspResultStatus::Stale,
        capabilities: Vec::new(),
        diagnostics_latency_ms: None,
        restart_count: 5,
        download_decision_id: None,
        schema_version: LspServerHealthRecord::schema_version(),
    };
    let p = project_lsp_health(&record, false);
    assert_eq!(p.version_label, "unknown");
    assert_eq!(p.provenance_label, "configured path");
    assert_eq!(p.restart_count, 5);
}
