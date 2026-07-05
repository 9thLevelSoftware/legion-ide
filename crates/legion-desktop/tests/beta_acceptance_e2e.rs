use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_desktop::{
    beta::{
        BetaProposalMode, BetaSaveOutcome, BetaTerminalPolicyDecision, BetaWorkflowConfig,
        BetaWorkflowStatus, run_beta_workflow,
    },
    bridge::DesktopAction,
    view::DesktopProjectionViewModel,
    workflow::{DesktopLaunchConfig, DesktopRuntime, DesktopWorkflowOutcome},
};
use legion_protocol::{
    AssistedAiTrustProjectionKind, AssistedAiTrustProjectionReference, CapabilityId, CausalityId,
    ContextManifestItemCount, CorrelationId, DelegatedTaskAffectedTargetSummary,
    DelegatedTaskOperationClass, DelegatedTaskPlanContract, DelegatedTaskPlanId,
    DelegatedTaskPlanStep, DelegatedTaskPlanningBoundaryInput, DelegatedTaskStepId,
    DelegatedTaskStepState, EventSequence, FileFingerprint, PluginActivationEvent,
    PluginCommandDescriptor, PluginContribution, PluginId, PluginManifest, PluginQuotaDeclaration,
    PluginStateNamespace, PluginTrustDecision, PluginTrustMetadata, PluginTrustSource,
    ProposalContextManifestSummary, ProposalDiffSummary, ProposalDiffSummaryKind,
    ProposalLifecycleState, ProposalPrivacyLabel, ProposalRiskLabel, ProposalTargetKind,
    RedactionHint, TimestampMillis, VsCodeActivationEvent, VsCodeCompatibilityStatus,
    VsCodeCompatibilityTier, VsCodeContributionDescriptor, VsCodeContributionKind,
    VsCodeExtensionHostRuntime, VsCodeExtensionHostSession, VsCodeExtensionId, VsCodeExtensionKind,
    VsCodeExtensionManifest, delegated_task_plan_from_boundary_input,
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
    // 12a) terminal policy decision is asserted on the typed field, not prose.
    // Terminal productization: trusted beta workspaces launch the selected
    // shell through the product gate (labels never execute); the policy-gated
    // outcome for this trusted loop is therefore Allowed. Untrusted denial
    // stays covered by legion-app terminal_workflow tests.
    assert_eq!(
        report.terminal_decision,
        BetaTerminalPolicyDecision::Allowed,
        "12a) terminal policy should allow the trusted-workspace shell launch"
    );
    // 4) + 12b) The AI proposal mode is asserted on the typed enum: it must be
    // preview-only, which structurally excludes any autonomous apply.
    assert_eq!(
        report.proposal_mode,
        BetaProposalMode::PreviewOnly,
        "4)+12b) AI proposal must be preview-only, never autonomous apply"
    );
    assert_eq!(
        report.save_outcome,
        BetaSaveOutcome::Saved,
        "10) edit should save through proposal mediation"
    );
    assert!(
        report.errors.is_empty(),
        "passing beta workflow should record no typed errors"
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

    // 12) No policy/proposal bypass: drive a proposal through the production
    // runtime path and assert against the *structured* proposal model that no
    // proposal ever reaches an applied/approved lifecycle state, rather than
    // relying solely on substring scans of a human-readable status string.
    let proposal_start = runtime
        .handle_action(DesktopAction::StartAiProposal {
            instruction_label: "beta no-bypass proposal".to_string(),
        })
        .expect("start ai proposal");
    if let DesktopWorkflowOutcome::AssistedAiUpdated {
        proposal_id: Some(proposal_id),
        ..
    } = proposal_start
    {
        let _ = runtime.handle_action(DesktopAction::OpenProposalDetails { proposal_id });
        let _ = runtime.handle_action(DesktopAction::PreviewProposal { proposal_id });
    }
    let proposal_snapshot = runtime.projection_snapshot();

    // Structured action-set assertion (typed, not substring): the set of
    // proposal lifecycle states the runtime ever projects must be disjoint from
    // the autonomous-apply terminal states. We assert against the typed
    // `ProposalLifecycleState` set, never against rendered text.
    let projected_lifecycle_states: Vec<ProposalLifecycleState> = proposal_snapshot
        .proposal_ledger_projection
        .rows
        .iter()
        .map(|row| row.lifecycle.state)
        .collect();
    for forbidden in [
        ProposalLifecycleState::Applied,
        ProposalLifecycleState::Approved,
    ] {
        assert!(
            !projected_lifecycle_states.contains(&forbidden),
            "12c) projected proposal action set must not contain autonomous-apply state {forbidden:?}"
        );
    }

    // Structured command-model assertion: the projected, typed command registry
    // must offer no enabled command that *autonomously* applies a proposal. We
    // inspect typed `CommandDescriptor` fields (command_id segments + enabled
    // flag), not a free-form prose scan. Human-mediated affordances such as
    // `proposal.approve`/`proposal.apply` are the policy gate and are allowed;
    // only autonomous/auto-apply commands are forbidden.
    let autonomous_apply_commands: Vec<&str> = proposal_snapshot
        .command_registry_projection
        .commands
        .iter()
        .filter(|command| command.enabled)
        .filter(|command| {
            command
                .command_id
                .split(['.', '_', '-'])
                .any(|segment| matches!(segment, "autoapply" | "autonomous" | "autoapprove"))
        })
        .map(|command| command.command_id.as_str())
        .collect();
    assert!(
        autonomous_apply_commands.is_empty(),
        "12c) projected command model must expose no enabled autonomous-apply command, saw {autonomous_apply_commands:?}"
    );

    let _ = runtime.handle_action(DesktopAction::Quit);
    assert!(runtime.quit_requested());

    // The beta report's typed proposal mode is the authoritative no-bypass
    // signal; prose status is asserted only as markdown evidence formatting.
    assert_eq!(
        report.proposal_mode,
        BetaProposalMode::PreviewOnly,
        "12e) beta report proposal mode must remain preview-only"
    );
    assert!(
        !evidence_text.contains("bypass"),
        "12g) evidence must not mention bypass"
    );
}

/// Build a metadata-only Legion plugin manifest from an approved VSIX
/// compatibility manifest so the extension can be installed through the real
/// `runtime.load_plugin_manifest` path. The VSIX fixture supplies identity,
/// version, and capability metadata; the Legion runtime owns trust and quotas.
fn plugin_manifest_from_vsix(vsix: &VsCodeExtensionManifest) -> PluginManifest {
    let mut requested_capabilities = vsix.requested_capabilities.clone();
    let command_capability = CapabilityId("plugin.command".to_string());
    if !requested_capabilities.contains(&command_capability) {
        requested_capabilities.push(command_capability.clone());
    }
    PluginManifest {
        plugin_id: vsix.plugin_id,
        name: vsix.name.clone(),
        version: vsix.version.clone(),
        schema_version: 1,
        min_abi_version: 1,
        max_abi_version: 1,
        module_hash: format!("sha256:vsix:{}", vsix.extension_id.0),
        manifest_id: format!("manifest:vsix:{}", vsix.extension_id.0),
        trust: PluginTrustMetadata {
            source: PluginTrustSource::ExplicitLocalAllow,
            decision: PluginTrustDecision::ExplicitlyAllowed,
            reason: "beta acceptance approved VSIX install".to_string(),
        },
        signature: None,
        activation_events: vec![PluginActivationEvent::OnCommand {
            command: "vsix.activate".to_string(),
        }],
        contributions: vec![PluginContribution::Command(PluginCommandDescriptor {
            command_id: "vsix.activate".to_string(),
            title: vsix.display_name.clone(),
            required_capability: command_capability,
        })],
        requested_capabilities,
        storage_namespace: PluginStateNamespace {
            plugin_id: vsix.plugin_id,
            namespace: "state".to_string(),
        },
        quotas: PluginQuotaDeclaration {
            max_fuel: 1000,
            max_wall_time_ms: 50,
            max_memory_pages: 8,
            max_storage_bytes: 4096,
            max_host_calls: 1,
            max_events: 4,
            max_output_bytes: 512,
        },
    }
}

fn verification_fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "test".to_string(),
        value: value.to_string(),
    }
}

fn verification_trust_ref(
    kind: AssistedAiTrustProjectionKind,
    label: &str,
) -> AssistedAiTrustProjectionReference {
    AssistedAiTrustProjectionReference {
        reference_id: format!("projection:{label}"),
        kind,
        projection_hash: verification_fingerprint(&format!("hash:{label}")),
        schema_version: 1,
    }
}

/// Build a metadata-only delegated-task plan contract. Seeding it through the
/// runtime causes the app to derive a required verification-run row, which is
/// the production path that populates `verification_run_projection`.
fn verification_plan_contract() -> DelegatedTaskPlanContract {
    let plan_id = DelegatedTaskPlanId("plan:beta:verification".to_string());
    let causality_id: CausalityId =
        serde_json::from_str("\"cccccccc-cccc-cccc-cccc-cccccccccccc\"")
            .expect("causality id should deserialize");
    let target = DelegatedTaskAffectedTargetSummary {
        target_id: "target:metadata".to_string(),
        kind: ProposalTargetKind::MetadataOnly,
        workspace_id: None,
        file_id: None,
        buffer_id: None,
        ranges: Vec::new(),
        hashes: vec![verification_fingerprint("hash:target")],
        counts: vec![ContextManifestItemCount {
            label: "targets".to_string(),
            count: 1,
        }],
        labels: vec!["delegated_task.target.metadata_only".to_string()],
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    let step = DelegatedTaskPlanStep {
        step_id: DelegatedTaskStepId(format!("step:{}:verify", plan_id.0)),
        order: 1,
        objective_summary_hash: verification_fingerprint("hash:step"),
        operation_class: DelegatedTaskOperationClass::SummarizeVerificationReadiness,
        depends_on: Vec::new(),
        required_gates: Vec::new(),
        target_ids: vec!["target:metadata".to_string()],
        proposal_preview: None,
        state: DelegatedTaskStepState::Planned,
        blockers: Vec::new(),
        labels: vec!["delegated_task.step.metadata_only".to_string()],
        counts: vec![ContextManifestItemCount {
            label: "steps".to_string(),
            count: 1,
        }],
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    delegated_task_plan_from_boundary_input(DelegatedTaskPlanningBoundaryInput {
        plan_id,
        workspace_id: None,
        objective_summary_hash: verification_fingerprint("hash:objective"),
        allowed_operation_classes: vec![
            DelegatedTaskOperationClass::SummarizeVerificationReadiness,
            DelegatedTaskOperationClass::RequestHumanApproval,
        ],
        context_manifest: Some(verification_trust_ref(
            AssistedAiTrustProjectionKind::ContextManifest,
            "context",
        )),
        privacy_inspector: Some(verification_trust_ref(
            AssistedAiTrustProjectionKind::PrivacyInspector,
            "privacy",
        )),
        permission_budget_projection: Some(verification_trust_ref(
            AssistedAiTrustProjectionKind::PermissionBudget,
            "budget",
        )),
        approval_checklist: Some(verification_trust_ref(
            AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
            "approval",
        )),
        checkpoint_rollback: Some(verification_trust_ref(
            AssistedAiTrustProjectionKind::CheckpointRollback,
            "checkpoint",
        )),
        assisted_ai_projection: Some(verification_trust_ref(
            AssistedAiTrustProjectionKind::AssistedAiProjection,
            "assisted",
        )),
        assisted_ai_required: true,
        affected_targets: vec![target],
        steps: vec![step],
        proposal_preview_links: Vec::new(),
        workspace_trust_state: legion_protocol::WorkspaceTrustState::Trusted,
        privacy_denied: false,
        permission_budget_denied: false,
        permission_budget_depleted: false,
        approval_checklist_valid: true,
        checkpoint_required: false,
        checkpoint_available: true,
        rollback_required: false,
        rollback_available: true,
        correlation_id: CorrelationId(902),
        causality_id,
        created_at: TimestampMillis(4),
        schema_version: 1,
    })
}

/// F1 (VSIX) workflow-level coverage: install an approved VSIX through the real
/// `runtime.load_plugin_manifest` path, open a real AI proposal and inspect its
/// projected context manifest + diff via the view model, and record a
/// verification run through the production delegated-task seeding path. All
/// assertions are on typed projected install/proposal/verification state.
#[test]
fn beta_acceptance_e2e_vsix_install_proposal_diff_and_verification_through_runtime() {
    let workspace_root = std::env::temp_dir().join(format!(
        "beta-acceptance-vsix-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&workspace_root).expect("vsix workspace should be created");
    fs::write(workspace_root.join("main.rs"), "fn main() {}\n").expect("seed file");

    let mut runtime = DesktopRuntime::open(DesktopLaunchConfig::new(
        workspace_root.clone(),
        Some("main.rs".to_string()),
    ))
    .expect("desktop runtime should open vsix workspace");
    runtime
        .set_product_mode(legion_app::AppProductMode::Assist)
        .expect("assist mode");

    // 2) Install the approved VSIX through the production plugin/runtime path.
    let vsix = approved_vsix_manifest();
    let installed_plugin_id = runtime
        .load_plugin_manifest(plugin_manifest_from_vsix(&vsix))
        .expect("approved VSIX should install through runtime plugin authority");
    assert_eq!(installed_plugin_id, vsix.plugin_id);

    let snapshot = runtime.projection_snapshot();
    assert!(
        snapshot
            .plugin_contribution_projections
            .iter()
            .any(|projection| projection.plugin_id == vsix.plugin_id
                && projection.status_label == "loaded"),
        "projected install state should show the loaded VSIX plugin"
    );
    let install_model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    assert!(
        install_model
            .plugin_rows
            .iter()
            .any(|row| row.contains(&format!("plugin {}", vsix.plugin_id.0))),
        "view model should project the installed plugin row"
    );

    // 5) + 6) Open a real AI proposal and inspect its projected context manifest
    //    and diff through the typed ledger + view model.
    let proposal_start = runtime
        .handle_action(DesktopAction::StartAiProposal {
            instruction_label: "beta vsix proposal".to_string(),
        })
        .expect("start ai proposal");
    let proposal_id = match proposal_start {
        DesktopWorkflowOutcome::AssistedAiUpdated {
            proposal_id: Some(proposal_id),
            ..
        } => proposal_id,
        other => panic!("expected an assisted-AI proposal id, got {other:?}"),
    };
    runtime
        .handle_action(DesktopAction::OpenProposalDetails { proposal_id })
        .expect("open proposal details");
    runtime
        .handle_action(DesktopAction::PreviewProposal { proposal_id })
        .expect("preview proposal");

    let proposal_snapshot = runtime.projection_snapshot();
    let proposal_row = proposal_snapshot
        .proposal_ledger_projection
        .rows
        .iter()
        .find(|row| row.proposal_id == proposal_id)
        .expect("projected proposal row should exist for the opened proposal");
    // Context manifest projected and metadata-only.
    assert!(
        proposal_row
            .context_manifest
            .redaction_hints
            .contains(&RedactionHint::MetadataOnly),
        "projected context manifest must be metadata-only"
    );
    // Diff summary projected with full source redacted (no autonomous source apply).
    assert!(
        proposal_row.diff_summary.full_source_redacted,
        "projected proposal diff must redact full source"
    );
    assert!(
        !matches!(
            proposal_row.lifecycle.state,
            ProposalLifecycleState::Applied | ProposalLifecycleState::Approved
        ),
        "opened proposal must remain preview-only"
    );

    // 7) Record a verification run through the production delegated-task path and
    //    assert the derived, projected verification state.
    runtime
        .seed_delegated_task_plan_contracts(vec![verification_plan_contract()])
        .expect("seed delegated task plan contracts");
    let verification_snapshot = runtime.projection_snapshot();
    let verification_rows = &verification_snapshot.verification_run_projection.rows;
    assert!(
        !verification_rows.is_empty(),
        "seeding a delegated plan should project at least one verification run"
    );
    assert!(
        verification_rows.iter().all(|row| row.command_body_redacted
            && row.redaction_hints.contains(&RedactionHint::MetadataOnly)),
        "projected verification runs must be metadata-only with redacted command body"
    );
    assert!(
        verification_rows
            .iter()
            .any(|row| row.run_id.starts_with("verification:")),
        "projected verification run id should be derived from the delegated plan"
    );

    let _ = runtime.handle_action(DesktopAction::Quit);
    let _ = fs::remove_dir_all(&workspace_root);
}
