use legion_desktop::view::DesktopProjectionViewModel;
use legion_protocol::{
    LanguageId, LanguageServerId, LspResultStatus, LspServerBinaryProvenance, LspServerHealthRecord,
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
fn health_projection_all_provenance_variants_have_exact_labels() {
    let base = LspServerHealthRecord {
        server_id: LanguageServerId(7),
        language_id: LanguageId("rust".into()),
        binary_provenance: LspServerBinaryProvenance::Configured,
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
        (LspServerBinaryProvenance::Configured, "configured path"),
        (LspServerBinaryProvenance::ProjectLocal, "project-local"),
        (LspServerBinaryProvenance::SystemPath, "system PATH"),
        (LspServerBinaryProvenance::Bundled, "bundled"),
        (LspServerBinaryProvenance::Downloaded, "downloaded"),
    ];
    for (provenance, expected_label) in cases {
        let mut record = base.clone();
        record.binary_provenance = provenance;
        let p = project_lsp_health(&record, false);
        assert_eq!(
            p.provenance_label, expected_label,
            "provenance {:?} should map to label '{}'",
            provenance, expected_label
        );
    }
}

#[test]
fn health_projection_server_label_uses_language_id() {
    let record = LspServerHealthRecord {
        server_id: LanguageServerId(42),
        language_id: LanguageId("python".into()),
        binary_provenance: LspServerBinaryProvenance::SystemPath,
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
    let p = project_lsp_health(&record, false);
    assert_eq!(p.server_label, "python#42");
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

/// M4: `lsp_health_rows` (via `DesktopProjectionViewModel`) emits a
/// well-formed formatted string containing server, provenance, version, status,
/// and restart fields that the projection snapshot has populated.
#[test]
fn m4_lsp_health_rows_formatted_output() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = legion_app::AppComposition::new();
    app.open_workspace(
        dir.path(),
        legion_protocol::WorkspaceTrustState::Trusted,
        legion_protocol::PrincipalId("test".to_string()),
    )
    .expect("open workspace");

    let health = LspServerHealthRecord {
        server_id: LanguageServerId(1),
        language_id: LanguageId("rust".into()),
        binary_provenance: LspServerBinaryProvenance::Configured,
        binary_path_hash: None,
        artifact_hash: None,
        version: Some("1.77.0".into()),
        init_status: LspResultStatus::Fresh,
        capabilities: Vec::new(),
        diagnostics_latency_ms: None,
        restart_count: 2,
        download_decision_id: None,
        schema_version: LspServerHealthRecord::schema_version(),
    };
    app.set_lsp_health_for_test(health);

    let snapshot = app
        .shell_projection_snapshot("test")
        .expect("snapshot must succeed");
    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(
        !model.lsp_health_rows.is_empty(),
        "lsp_health_rows must be non-empty when a health record is injected"
    );

    let row = &model.lsp_health_rows[0];
    assert!(
        row.contains("lsp server="),
        "row must contain 'lsp server=': {row:?}"
    );
    assert!(
        row.contains("provenance="),
        "row must contain 'provenance=': {row:?}"
    );
    assert!(
        row.contains("version="),
        "row must contain 'version=': {row:?}"
    );
    assert!(
        row.contains("status="),
        "row must contain 'status=': {row:?}"
    );
    assert!(
        row.contains("restarts="),
        "row must contain 'restarts=': {row:?}"
    );
    assert!(
        row.contains("1.77.0"),
        "row must contain the injected version '1.77.0': {row:?}"
    );
    assert!(
        row.contains("ready"),
        "row must contain 'ready' (Fresh status label): {row:?}"
    );
    assert!(
        row.contains("restarts=2"),
        "row must contain 'restarts=2' (injected restart count): {row:?}"
    );
}
