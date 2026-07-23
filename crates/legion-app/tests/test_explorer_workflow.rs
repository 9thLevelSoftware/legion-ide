//! P2.F3.T4–T5b — cargo test discovery, run, evidence, and workflow attach.

use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

use legion_app::test_explorer::{
    parse_cargo_test_list, parse_cargo_test_summary, projection_from_items, validate_test_item_id,
};
use legion_app::{AppCommandOutcome, AppComposition};
use legion_protocol::{
    ByteRange, CausalityId, CommandRiskLabel, CorrelationId, DelegatedTaskAffectedTargetSummary,
    DelegatedTaskOperationClass, FileFingerprint, LanguageCodeLensProjection,
    LegionWorkflowModelBackend, LegionWorkflowSession, LegionWorkflowSessionId,
    LegionWorkflowState, LegionWorkflowWorkerAssignment, LegionWorkflowWorkerId,
    LegionWorkflowWorkerRole, LegionWorkflowWorkerState, PrincipalId, PrivacyClassification,
    ProductMode, ProposalPrivacyLabel, ProposalRiskLabel, ProposalTargetKind, RedactionHint,
    TimestampMillis, WorkspaceId, WorkspaceTrustState,
};
use legion_ui::CommandDispatchIntent;
use uuid::Uuid;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_fixture_crate() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "legion-test-explorer-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "legion_test_explorer_fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write Cargo.toml");
    fs::write(
        root.join("src/lib.rs"),
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn fixture_ok() {}\n}\n",
    )
    .expect("write lib");
    root
}

#[test]
fn test_explorer_refresh_requires_workspace() {
    let mut app = AppComposition::new();
    let err = app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshTestExplorer)
        .expect_err("must fail closed without workspace");
    let _ = format!("{err}");
}

#[test]
fn test_explorer_parse_and_projection_are_metadata_only() {
    let items =
        parse_cargo_test_list("crate::alpha: test\ncrate::beta: bench\n\n2 tests, 1 benchmarks\n");
    assert_eq!(items.len(), 2);
    let projection = projection_from_items(items, Vec::new(), "ready", TimestampMillis(7));
    assert_eq!(projection.schema_version, 1);
    assert_eq!(projection.controller_label, "cargo-test");
    assert_eq!(projection.items[0].item_id, "crate::alpha");
    assert!(projection.diagnostics.is_empty());
}

#[test]
fn test_explorer_refresh_discovers_fixture_or_reports_honest_error() {
    let root = create_fixture_crate();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-test-explorer".to_string()),
    )
    .expect("open workspace");

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshTestExplorer)
        .expect("refresh should not panic");
    match outcome {
        AppCommandOutcome::TestExplorerUpdated(projection) => {
            assert_eq!(projection.controller_label, "cargo-test");
            assert!(!projection.status_label.is_empty());
            assert!(projection.items.len() <= 500);
            if projection.status_label == "ready" {
                assert!(
                    projection
                        .items
                        .iter()
                        .any(|item| item.item_id.contains("fixture_ok")
                            || item.label.contains("fixture_ok")),
                    "expected fixture_ok among {:?}",
                    projection
                        .items
                        .iter()
                        .map(|i| i.item_id.as_str())
                        .collect::<Vec<_>>()
                );
            }
            let snap = app
                .shell_projection_snapshot("test-explorer")
                .expect("snapshot");
            assert_eq!(
                snap.test_explorer_projection.status_label,
                projection.status_label
            );
        }
        other => panic!("unexpected outcome: {other:?}"),
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_explorer_prefers_lsp_runnables_over_cargo_list() {
    let root = create_fixture_crate();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-test-explorer-lsp".to_string()),
    )
    .expect("open workspace");

    // Inject a runnable code lens into language tooling projection.
    app.inject_test_code_lenses_for_tests(vec![LanguageCodeLensProjection {
        lens_id: "lens-run-1".to_string(),
        title: "Run Test fixture_ok".to_string(),
        command_label: "cargo test fixture_ok".to_string(),
        kind_label: "runnable".to_string(),
        range: None,
        data_label: None,
        source_label: "rust-analyzer".to_string(),
        schema_version: 1,
    }]);

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshTestExplorer)
        .expect("refresh");
    match outcome {
        AppCommandOutcome::TestExplorerUpdated(projection) => {
            assert_eq!(projection.controller_label, "lsp-runnable");
            assert_eq!(projection.items.len(), 1);
            assert_eq!(projection.items[0].item_id, "lens-run-1");
            assert!(
                projection
                    .diagnostics
                    .iter()
                    .any(|d| d.contains("lsp-runnable"))
            );
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_explorer_run_rejects_invalid_item_id() {
    let root = create_fixture_crate();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-test-explorer-run".to_string()),
    )
    .expect("open workspace");
    let err = app
        .dispatch_ui_intent(CommandDispatchIntent::RunTestExplorerItem {
            item_id: "evil;rm -rf".to_string(),
        })
        .expect_err("must reject unsafe item ids");
    let msg = format!("{err}");
    assert!(msg.contains("invalid test item id"), "msg={msg}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_explorer_run_fixture_records_last_run_and_verification_row() {
    assert!(validate_test_item_id("tests::fixture_ok").is_ok());
    let (p, f, _, ok) = parse_cargo_test_summary(
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n",
    );
    assert!(ok && p == 1 && f == 0);

    let root = create_fixture_crate();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-test-explorer-run2".to_string()),
    )
    .expect("open workspace");

    // Discover first so items exist; run may still work without list if id is known.
    let _ = app
        .dispatch_ui_intent(CommandDispatchIntent::RefreshTestExplorer)
        .expect("refresh");

    let item_id = "tests::fixture_ok".to_string();
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::RunTestExplorerItem {
            item_id: item_id.clone(),
        })
        .expect("run should not panic");
    match outcome {
        AppCommandOutcome::TestExplorerUpdated(projection) => {
            assert_eq!(
                projection.last_run_item_id.as_deref(),
                Some(item_id.as_str())
            );
            assert!(projection.last_run_status.is_some());
            // When cargo is available, fixture_ok should pass.
            if projection.last_run_status.as_deref() == Some("passed") {
                assert_eq!(projection.last_run_exit_code, Some(0));
            }
            let snap = app
                .shell_projection_snapshot("test-explorer-run")
                .expect("snapshot");
            assert!(
                snap.verification_run_projection
                    .rows
                    .iter()
                    .any(|row| row.command_class_label == "cargo-test-exact"
                        && row.target_labels.iter().any(|t| t == &item_id)
                        && row.evidence_artifact_id.is_some()),
                "expected verification row for exact run, got {:?}",
                snap.verification_run_projection.rows
            );
            assert!(
                !app.test_explorer_run_summaries().is_empty(),
                "expected protocol test-run summaries for agent evidence"
            );
            let artifacts = app
                .test_explorer_evidence_artifacts()
                .expect("evidence artifacts");
            assert!(!artifacts.is_empty());
            assert!(artifacts.iter().all(|a| !a.raw_payload_retained));
            assert!(
                artifacts
                    .iter()
                    .all(|a| a.artifact_id.starts_with("artifact:evidence:test-run:"))
            );
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    let _ = fs::remove_dir_all(&root);
}

fn minimal_workflow_session(label: &str) -> LegionWorkflowSession {
    let worker = LegionWorkflowWorkerAssignment {
        worker_id: LegionWorkflowWorkerId(format!("worker:{label}")),
        role: LegionWorkflowWorkerRole::Implementer,
        state: LegionWorkflowWorkerState::Ready,
        model_backend: LegionWorkflowModelBackend::Local,
        display_safe_model_label: format!("model:{label}"),
        allowed_command_classes: vec![DelegatedTaskOperationClass::DraftProposalMetadata],
        linked_delegated_plan_id: None,
        assisted_ai_route: None,
        affected_targets: vec![DelegatedTaskAffectedTargetSummary {
            target_id: format!("target:{label}"),
            kind: ProposalTargetKind::MetadataOnly,
            workspace_id: Some(WorkspaceId(1)),
            file_id: None,
            buffer_id: None,
            ranges: vec![ByteRange::new(0, 0)],
            hashes: vec![FileFingerprint {
                algorithm: "sha256".to_string(),
                value: format!("hash:{label}"),
            }],
            counts: Vec::new(),
            labels: vec![format!("target:{label}")],
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        risk_labels: vec![CommandRiskLabel::Review],
        privacy_labels: vec![PrivacyClassification::Metadata],
        correlation_id: CorrelationId(31),
        causality_id: CausalityId(Uuid::from_u128(31)),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    LegionWorkflowSession {
        session_id: LegionWorkflowSessionId(format!("session:{label}")),
        directive_artifact_id: Some(format!("directive:{label}")),
        spec_artifact_id: Some(format!("spec:{label}")),
        task_graph_artifact_id: Some(format!("task-graph:{label}")),
        product_mode: ProductMode::LegionWorkflows,
        worker_assignments: vec![worker],
        dependency_edges: Vec::new(),
        conflict_summaries: Vec::new(),
        verification_gates: Vec::new(),
        sign_off_records: Vec::new(),
        proposal_ids: Vec::new(),
        merge_approval: None,
        lifecycle_state: LegionWorkflowState::Executing,
        generated_at: TimestampMillis(1),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
        correlation_id: CorrelationId(13),
        causality_id: CausalityId(Uuid::from_u128(13)),
    }
}

#[test]
fn test_explorer_attach_and_export_includes_agent_evidence() {
    let root = create_fixture_crate();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-test-explorer-attach".to_string()),
    )
    .expect("open workspace");

    // Produce at least one explorer run summary.
    let _ = app
        .dispatch_ui_intent(CommandDispatchIntent::RunTestExplorerItem {
            item_id: "tests::fixture_ok".to_string(),
        })
        .expect("run fixture");
    assert!(!app.test_explorer_run_summaries().is_empty());

    let session = minimal_workflow_session("test-explorer-attach");
    let session_id = session.session_id.clone();
    app.seed_legion_workflow_sessions(vec![session])
        .expect("seed workflow");

    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::AttachTestExplorerEvidence {
            session_id: session_id.0.clone(),
        })
        .expect("attach evidence");
    match outcome {
        AppCommandOutcome::TestExplorerUpdated(projection) => {
            assert!(
                projection
                    .diagnostics
                    .iter()
                    .any(|d| d.contains("attached-evidence:") && d.contains("count=")),
                "diagnostics={:?}",
                projection.diagnostics
            );
        }
        other => panic!("unexpected outcome: {other:?}"),
    }

    let records = app
        .test_explorer_legion_evidence_records()
        .expect("legion records");
    assert!(!records.is_empty());
    assert!(
        records
            .iter()
            .all(|r| r.evidence_id.contains("test-explorer")
                && r.redaction_hints.contains(&RedactionHint::MetadataOnly))
    );

    let bundle = app
        .export_legion_workflow_evidence_bundle(&session_id)
        .expect("export bundle");
    assert!(
        bundle
            .evidence_records
            .iter()
            .any(|r| r.evidence_id.contains("test-run") || r.command_label.is_some()),
        "export should include explorer evidence, got {:?}",
        bundle
            .evidence_records
            .iter()
            .map(|r| r.evidence_id.as_str())
            .collect::<Vec<_>>()
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_explorer_run_group_records_filter_mode_and_summary() {
    let root = create_fixture_crate();
    let mut app = AppComposition::new();
    app.open_workspace(
        &root,
        WorkspaceTrustState::Trusted,
        PrincipalId("principal-test-explorer-group".to_string()),
    )
    .expect("open workspace");
    let outcome = app
        .dispatch_ui_intent(CommandDispatchIntent::RunTestExplorerGroup {
            parent_label: "tests".to_string(),
        })
        .expect("group run");
    match outcome {
        AppCommandOutcome::TestExplorerUpdated(projection) => {
            assert_eq!(projection.last_run_item_id.as_deref(), Some("tests"));
            assert!(
                projection
                    .diagnostics
                    .iter()
                    .any(|d| d.contains("filter-mode=group") || d.starts_with("last-run:tests:")),
                "diagnostics={:?}",
                projection.diagnostics
            );
            assert!(!app.test_explorer_run_summaries().is_empty());
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    let _ = fs::remove_dir_all(&root);
}
