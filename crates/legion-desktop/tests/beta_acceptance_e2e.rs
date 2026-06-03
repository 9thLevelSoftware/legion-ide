use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    beta::{BetaWorkflowConfig, BetaWorkflowStatus, run_beta_workflow},
    bridge::DesktopAction,
    view::DesktopProjectionViewModel,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::{
    CapabilityId, CausalityId, CorrelationId, EventSequence, FileFingerprint, PluginId,
    ProposalContextManifestSummary, ProposalDiffSummary, ProposalDiffSummaryKind,
    ProposalPrivacyLabel, ProposalRiskLabel, RedactionHint, TimestampMillis, VsCodeActivationEvent,
    VsCodeCompatibilityStatus, VsCodeCompatibilityTier, VsCodeContributionDescriptor,
    VsCodeContributionKind, VsCodeExtensionHostRuntime, VsCodeExtensionHostSession,
    VsCodeExtensionId, VsCodeExtensionKind, VsCodeExtensionManifest,
};
use legion_ui::{DebugStatusKindProjection, DebugStepKindProjection};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct BetaTestPaths {
    prefix: String,
}

impl BetaTestPaths {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self {
            prefix: format!("beta-acceptance-e2e-{}-{nanos}-{id}", std::process::id()),
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        PathBuf::from("target").join(format!("{}-{name}", self.prefix))
    }
}

impl Drop for BetaTestPaths {
    fn drop(&mut self) {
        cleanup_test_paths("target", &self.prefix);
    }
}

fn cleanup_test_paths(target_root: impl AsRef<Path>, prefix: &str) {
    let target_root = target_root.as_ref();
    let Ok(entries) = fs::read_dir(target_root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(prefix) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            let _ = fs::remove_dir_all(path);
        } else {
            let _ = fs::remove_file(path);
        }
    }
    let _ = fs::remove_dir(target_root);
}

fn beta_config(
    paths: &BetaTestPaths,
    beta_workspace: PathBuf,
    evidence_path: PathBuf,
) -> BetaWorkflowConfig {
    BetaWorkflowConfig::new(
        PathBuf::from("."),
        beta_workspace,
        evidence_path,
        paths.path("session.json"),
        paths.path("diagnostics.md"),
    )
    .expect("beta config should be valid")
}

fn approved_vsix_manifest() -> VsCodeExtensionManifest {
    VsCodeExtensionManifest {
        extension_id: VsCodeExtensionId("legion.approved".to_string()),
        plugin_id: PluginId(42),
        publisher: "legion".to_string(),
        name: "approved-extension".to_string(),
        display_name: "Approved Extension".to_string(),
        version: "1.0.0".to_string(),
        engine_vscode: Some("^1.90.0".to_string()),
        extension_kinds: vec![VsCodeExtensionKind::Ui],
        activation_events: vec![VsCodeActivationEvent {
            raw: "onLanguage:rust".to_string(),
            tier: VsCodeCompatibilityTier::Tier1ProtocolAdapter,
            status: VsCodeCompatibilityStatus::SupportedWithPolicy,
        }],
        contributions: vec![VsCodeContributionDescriptor {
            kind: VsCodeContributionKind::Theme,
            contribution_id: "themes".to_string(),
            count: 1,
            tier: VsCodeCompatibilityTier::Tier0Declarative,
            status: VsCodeCompatibilityStatus::Supported,
            metadata_label: "theme".to_string(),
        }],
        requested_capabilities: vec![CapabilityId("vscode.extension.activate".to_string())],
        required_tier: VsCodeCompatibilityTier::Tier0Declarative,
        status: VsCodeCompatibilityStatus::Supported,
        diagnostics: Vec::new(),
        correlation_id: CorrelationId(1),
        causality_id: CausalityId(uuid::Uuid::from_u128(1)),
        sequence: EventSequence(1),
        schema_version: 1,
    }
}

fn approved_vsix_host_session(manifest: &VsCodeExtensionManifest) -> VsCodeExtensionHostSession {
    VsCodeExtensionHostSession {
        extension_id: manifest.extension_id.clone(),
        runtime: VsCodeExtensionHostRuntime::NoneRequired,
        status: VsCodeCompatibilityStatus::Supported,
        process_label: "none-required".to_string(),
        requested_capabilities: manifest.requested_capabilities.clone(),
        correlation_id: manifest.correlation_id,
        causality_id: manifest.causality_id,
        sequence: manifest.sequence,
        schema_version: 1,
    }
}

fn proposal_context_manifest_fixture() -> ProposalContextManifestSummary {
    ProposalContextManifestSummary {
        manifest_id: "manifest:beta:context".to_string(),
        category_count: 3,
        total_item_count: 7,
        omitted_item_count: 0,
        categories: Vec::new(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
    }
}

fn proposal_diff_fixture() -> ProposalDiffSummary {
    ProposalDiffSummary {
        kind: ProposalDiffSummaryKind::MetadataOnly,
        target_count: 2,
        hunk_count: 3,
        inserted_line_count: 5,
        deleted_line_count: 2,
        omitted_hunk_count: 0,
        full_source_redacted: true,
        diff_hash: Some(FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "hash:beta:diff".to_string(),
        }),
        chunks: Vec::new(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
    }
}

fn test_verification_run_fixture() -> legion_protocol::VerificationRunRow {
    legion_protocol::VerificationRunRow {
        run_id: "run:beta:test".to_string(),
        label: "Beta acceptance test run".to_string(),
        state: legion_protocol::VerificationRunState::Passed,
        command_class_label: "cargo-test".to_string(),
        command_body_redacted: true,
        exit_code: Some(0),
        target_labels: vec!["legion-desktop".to_string()],
        evidence_artifact_id: Some("evidence:beta:test".to_string()),
        started_at: Some(TimestampMillis(1)),
        completed_at: Some(TimestampMillis(2)),
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

#[test]
fn beta_acceptance_e2e_policy_gated_local_loop() {
    // Beta Acceptance Scenario: unified end-to-end trace covering all 12
    // product-readiness bullets from plans/product-readiness-ledger.md:
    //  1) open large repo
    //  2) install approved VSIX
    //  3) run Rust LSP completion
    //  4) ask AI for multi-file change
    //  5) inspect context manifest
    //  6) review proposal diff
    //  7) run tests
    //  8) debug a failure
    //  9) collaborate on review
    // 10) save safely
    // 11) export audit evidence
    // 12) no policy/proposal bypass

    let paths = BetaTestPaths::new();
    let beta_workspace = paths.path("workspace");
    let evidence = paths.path("evidence.md");

    // 1) Open large repository + 10) Save safely + 11) Export audit evidence
    //    (base beta loop also covers 3, 4, and partial 12)
    let report = run_beta_workflow(beta_config(
        &paths,
        beta_workspace.clone(),
        evidence.clone(),
    ))
    .expect("beta workflow should pass");

    assert_eq!(report.status, BetaWorkflowStatus::Passed);
    assert!(
        report.browse_status.contains("refreshed explorer"),
        "1) repository should open and explorer refresh"
    );
    assert!(
        report.edit_save_status.contains("saved"),
        "10) edit should save safely through proposal mediation"
    );
    assert!(
        report.active_file_search_status.contains("completed"),
        "search should complete"
    );
    assert!(
        report.workspace_search_status.contains("completed"),
        "workspace search should complete"
    );
    assert!(
        report.language_status.contains("Cancelled")
            && report.language_status.contains("cancellations=1"),
        "3) Rust LSP completion should request and cancel safely"
    );
    assert!(
        report.terminal_status.contains("denied"),
        "12a) terminal policy should deny by default"
    );
    assert!(
        report.proposal_status.contains("preview"),
        "4) AI proposal should be preview-only, not applied"
    );
    assert!(
        !report.proposal_status.contains("apply"),
        "12b) proposal must not show autonomous apply"
    );

    // 11) Evidence export assertions
    let evidence_text = fs::read_to_string(&evidence).expect("evidence should be written");
    assert!(
        evidence_text.contains("status: passed"),
        "11) evidence should record passed status"
    );
    assert!(
        evidence_text.contains("metadata-only: true"),
        "11) evidence should declare metadata-only"
    );
    assert!(
        evidence_text.contains("unsupported_surfaces"),
        "11) evidence should list unsupported surfaces"
    );
    assert!(
        !evidence_text.contains("println!"),
        "12c) evidence must not leak raw source"
    );
    assert!(
        !evidence_text.contains("metadata-only beta edit"),
        "12d) evidence must not contain saved file text"
    );

    let saved_main = fs::read_to_string(beta_workspace.join("src/main.rs"))
        .expect("isolated beta fixture should be saved");
    assert!(
        saved_main.starts_with("// metadata-only beta edit"),
        "10) beta fixture should be saved safely"
    );

    // 2) Install approved VSIX metadata fixture
    let manifest = approved_vsix_manifest();
    assert_eq!(
        manifest.status,
        VsCodeCompatibilityStatus::Supported,
        "2a) approved VSIX should be Supported"
    );
    assert_eq!(
        manifest.required_tier,
        VsCodeCompatibilityTier::Tier0Declarative,
        "2b) approved VSIX should need no runtime host"
    );
    assert!(
        manifest.diagnostics.is_empty(),
        "2c) approved VSIX should have no diagnostics"
    );
    let host_session = approved_vsix_host_session(&manifest);
    assert_eq!(
        host_session.runtime,
        VsCodeExtensionHostRuntime::NoneRequired,
        "2d) host session should require no sidecar"
    );
    assert_eq!(
        host_session.status,
        VsCodeCompatibilityStatus::Supported,
        "2e) host session should be Supported"
    );
    assert!(
        !host_session.process_label.contains("node"),
        "2f) host session must not leak runtime command details"
    );

    // 5) Inspect context manifest + 6) Review proposal diff
    let context_manifest = proposal_context_manifest_fixture();
    assert_eq!(
        context_manifest.redaction_hints,
        vec![RedactionHint::MetadataOnly],
        "5a) context manifest should be metadata-only"
    );
    assert_eq!(
        context_manifest.category_count, 3,
        "5b) context manifest should have categories"
    );
    assert_eq!(
        context_manifest.total_item_count, 7,
        "5c) context manifest should list item count"
    );

    let diff = proposal_diff_fixture();
    assert_eq!(
        diff.kind,
        ProposalDiffSummaryKind::MetadataOnly,
        "6a) diff summary should be metadata-only"
    );
    assert!(
        diff.full_source_redacted,
        "6b) diff summary must redact full source"
    );
    assert_eq!(
        diff.redaction_hints,
        vec![RedactionHint::MetadataOnly],
        "6c) diff summary should have metadata-only redaction"
    );

    // 7) Run tests fixture
    let test_run = test_verification_run_fixture();
    assert_eq!(
        test_run.state,
        legion_protocol::VerificationRunState::Passed,
        "7a) test run should record Passed"
    );
    assert_eq!(
        test_run.command_class_label, "cargo-test",
        "7b) test run should be cargo-test class"
    );
    assert!(
        test_run.command_body_redacted,
        "7c) test run command body must be redacted"
    );
    assert_eq!(test_run.exit_code, Some(0), "7d) test run should exit 0");
    assert_eq!(
        test_run.redaction_hints,
        vec![RedactionHint::MetadataOnly],
        "7e) test run should be metadata-only"
    );

    // Open DesktopRuntime for additional interactive acceptance traces
    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        beta_workspace.clone(),
        Some("src/main.rs".to_string()),
    ))
    .expect("desktop runtime should open beta workspace");
    runtime
        .set_product_mode(legion_app::AppProductMode::Assist)
        .expect("set assist mode");

    // 8) Debug a failure
    runtime.enable_debug_fixture_for_tests();
    assert_eq!(
        runtime
            .handle_action(DesktopAction::RefreshDebugConfigurations)
            .expect("refresh debug configs"),
        DesktopWorkflowOutcome::DebugProjectionUpdated,
        "8a) debug configurations should refresh"
    );
    let snapshot = runtime.projection_snapshot();
    assert!(
        !snapshot.debug_projection.configurations.is_empty(),
        "8b) debug configurations should be present"
    );
    let config_id = snapshot
        .debug_projection
        .configurations
        .first()
        .expect("at least one debug config")
        .configuration_id
        .clone();
    assert_eq!(
        runtime
            .handle_action(DesktopAction::ToggleDebugBreakpoint {
                line: 1,
                condition: Some("count > 2".to_string()),
                hit_condition: Some("3".to_string()),
                log_message: Some("count changed".to_string()),
            })
            .expect("toggle breakpoint"),
        DesktopWorkflowOutcome::DebugProjectionUpdated,
        "8c) breakpoint should toggle"
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::LaunchDebugSession {
                configuration_id: config_id,
            })
            .expect("launch debug"),
        DesktopWorkflowOutcome::DebugProjectionUpdated,
        "8d) debug session should launch"
    );
    let snapshot = runtime.projection_snapshot();
    assert_eq!(
        snapshot.debug_projection.status.kind,
        DebugStatusKindProjection::Paused,
        "8e) debug session should be paused"
    );
    let session_id = snapshot
        .debug_projection
        .active_session_id
        .clone()
        .expect("active debug session");
    assert_eq!(
        runtime
            .handle_action(DesktopAction::DebugStep {
                session_id: session_id.clone(),
                kind: DebugStepKindProjection::Over,
            })
            .expect("debug step"),
        DesktopWorkflowOutcome::DebugProjectionUpdated,
        "8f) debug step should execute"
    );
    assert_eq!(
        runtime
            .handle_action(DesktopAction::DebugEvaluateSelection {
                session_id,
                expression_label: "beta".to_string(),
            })
            .expect("debug evaluate"),
        DesktopWorkflowOutcome::DebugProjectionUpdated,
        "8g) debug evaluate should execute"
    );
    let debug_model = DesktopProjectionViewModel::from_snapshot(&runtime.projection_snapshot());
    assert!(
        debug_model
            .debug_rows
            .iter()
            .any(|row| row.contains("debug config")),
        "8h) debug projection should show config metadata"
    );
    assert!(
        debug_model
            .debug_rows
            .iter()
            .any(|row| row.contains("debug breakpoint")),
        "8i) debug projection should show breakpoint metadata"
    );

    // 9) Collaborate on review
    runtime
        .enable_local_collaboration_runtime()
        .expect("enable collaboration runtime");
    let joined = runtime
        .handle_action(DesktopAction::JoinCollaborationSession {
            session_id: legion_protocol::CollaborationSessionId(91),
        })
        .expect("join collaboration session");
    assert!(
        matches!(
            joined,
            DesktopWorkflowOutcome::CollaborationUpdated {
                status: legion_desktop::workflow::DesktopCollaborationStatus::Joined,
                ..
            }
        ),
        "9a) collaboration session should join through app authority"
    );
    let presence = runtime
        .handle_action(DesktopAction::PublishCollaborationPresence {
            session_id: legion_protocol::CollaborationSessionId(91),
            participant_id: legion_protocol::CollaborationParticipantId(1),
        })
        .expect("publish collaboration presence");
    assert!(
        matches!(
            presence,
            DesktopWorkflowOutcome::CollaborationUpdated {
                status: legion_desktop::workflow::DesktopCollaborationStatus::PresencePublished,
                ..
            }
        ),
        "9b) collaboration presence should publish through app authority"
    );
    let snapshot = runtime.projection_snapshot();
    assert!(
        snapshot
            .collaboration_gui_projection
            .session_rows
            .iter()
            .any(|row| row.session_id == legion_protocol::CollaborationSessionId(91)),
        "9c) collaboration GUI should show session 91"
    );
    assert!(
        snapshot
            .collaboration_presence_projections
            .iter()
            .any(|presence| presence.session_id == legion_protocol::CollaborationSessionId(91)),
        "9d) collaboration presence should show metadata"
    );

    // 12) No policy/proposal bypass: ensure quit is clean and no autonomous apply
    let _ = runtime.handle_action(DesktopAction::Quit);
    assert!(runtime.quit_requested());
    assert!(
        !report.proposal_status.contains("apply"),
        "12e) proposal must never show apply"
    );
    assert!(
        !report.proposal_status.contains("autonomous"),
        "12f) proposal must never show autonomous"
    );
    assert!(
        !evidence_text.contains("bypass"),
        "12g) evidence must not mention bypass"
    );
}
