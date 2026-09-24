use super::*;
use crate::projection::{
    LegionWorkflowBoardColumnKind, LegionWorkflowBoardColumnProjection,
    LegionWorkflowBoardRowProjection, LegionWorkflowBudgetUsageRowProjection,
    LegionWorkflowFleetCardProjection,
};
use legion_protocol::{
    BufferId, BufferVersion, ByteRange, CanonicalPath, CapabilityId, FileFingerprint, FileId,
    LargeFileStatus, LegionWorkflowState, PermissionBudgetActionClass,
    PermissionBudgetConsentRequirementLabel, PermissionBudgetContract,
    PermissionBudgetResetPolicyLabel, PermissionBudgetState, PermissionBudgetUsageSummary,
    PrincipalId, ProposalContextManifestEntrySummary, ProposalContextManifestSummary,
    ProposalDiffChunkDescriptor, ProposalDiffSummary, ProposalDiffSummaryKind, ProposalLedgerRow,
    ProposalLifecycleState, ProposalLifecycleStateDisplay, ProposalPayloadKind,
    ProposalPrivacyLabel, ProposalRiskLabel, ProposalRollbackAvailability, ProposalTargetCoverage,
    ProposalTargetCoverageKind, ProtocolTextRange, RedactionHint, SnapshotId, Utf16Position,
    Utf16Range, ViewportDimensions, ViewportLineMetric, ViewportLineSlice,
    ViewportLineTruncationState, ViewportProjection, ViewportProjectionMode, ViewportScroll,
    WorkspaceId,
};

#[test]
fn panel_registry_filters_restricted_panels_out_of_manual_mode() {
    let registry = PanelRegistry::standard();
    let manual = registry.visible_for(DockMode::Manual);

    assert!(!manual.is_empty());
    assert!(
        manual.iter().all(|panel| !panel.requires_ai),
        "manual mode must not construct restricted panels: {manual:?}"
    );
    assert!(registry.is_visible_in(PanelId::ProjectExplorer, DockMode::Manual));
    assert!(registry.is_visible_in(PanelId::Terminal, DockMode::Manual));
    assert!(registry.is_visible_in(PanelId::PluginManager, DockMode::Manual));
    assert!(registry.is_visible_in(PanelId::Settings, DockMode::Manual));
    assert!(!registry.is_visible_in(PanelId::Assistant, DockMode::Manual));
    assert!(!registry.is_visible_in(PanelId::Delegation, DockMode::Manual));
    assert!(!registry.is_visible_in(PanelId::ApprovalQueue, DockMode::Manual));
    assert!(!registry.is_visible_in(PanelId::AgentFleet, DockMode::Manual));
    assert!(!registry.is_visible_in(PanelId::DecisionFeed, DockMode::Manual));
    assert!(!registry.is_visible_in(PanelId::Workflow, DockMode::Manual));
    assert!(!registry.is_visible_in(PanelId::Collaboration, DockMode::Manual));
    assert!(!registry.is_visible_in(PanelId::RemoteWorkspace, DockMode::Manual));
    assert!(registry.is_visible_in(PanelId::Assistant, DockMode::Assist));
    assert!(!registry.is_visible_in(PanelId::Delegation, DockMode::Assist));
    assert!(registry.is_visible_in(PanelId::Delegation, DockMode::Delegate));
    assert!(registry.is_visible_in(PanelId::Collaboration, DockMode::Delegate));
    assert!(!registry.is_visible_in(PanelId::RemoteWorkspace, DockMode::Delegate));
    assert!(registry.is_visible_in(PanelId::AgentFleet, DockMode::Automate));
    assert!(registry.is_visible_in(PanelId::RemoteWorkspace, DockMode::Automate));
}

#[test]
fn dock_panel_descriptor_roundtrips_projection_state() {
    let mut panel = DockPanelDescriptor::new(
        PanelId::Diagnostics,
        "Problems",
        "alert",
        DockSide::Bottom,
        false,
    );

    let state = panel.persist_state();
    assert_eq!(state["id"], "diagnostics");
    let expected: Vec<PanelCapability> = vec![PanelCapability::ManualIde];
    assert_eq!(panel.capabilities, expected);
    panel
        .restore_state(state)
        .expect("descriptor state restores");

    let error = panel
        .restore_state(serde_json::json!({
            "id": "assistant",
            "schema_version": 1,
        }))
        .expect_err("state for another panel is rejected");
    assert!(matches!(
        error,
        DockPanelStateError::InvalidState { message } if message.contains("does not match")
    ));
}

#[test]
fn dock_persisted_ids_parse_for_session_restore() {
    assert_eq!(DockMode::parse("Manual"), Some(DockMode::Manual));
    assert_eq!(
        DockMode::parse("Legion Workflows"),
        Some(DockMode::Automate)
    );
    assert_eq!(DockSide::parse("Right"), Some(DockSide::Right));
    assert_eq!(
        PanelId::parse("approval_queue"),
        Some(PanelId::ApprovalQueue)
    );
    assert_eq!(PanelId::parse("settings"), Some(PanelId::Settings));
    assert_eq!(PanelId::parse("unknown_panel"), None);
}

#[test]
fn dock_mode_labels_are_canonical() {
    assert_eq!(DockMode::Manual.label(), "Manual");
    assert_eq!(DockMode::Assist.label(), "Assist");
    assert_eq!(DockMode::Delegate.label(), "Delegate");
    assert_eq!(DockMode::Automate.label(), "Legion Workflows");
    assert_eq!(
        DockMode::Automate.to_product_mode(),
        ProductMode::LegionWorkflows
    );

    for legacy_label in [
        "Automate",
        "Autonomous",
        "LegionWorkflows",
        "Legion Workflows",
    ] {
        assert_eq!(
            DockMode::parse(legacy_label),
            Some(DockMode::Automate),
            "legacy label {legacy_label} should retain the Automate compatibility binding"
        );
    }
}

#[test]
fn mode_command_dispatches_canonical_legion_workflows_after_normalization() {
    let mut shell = Shell::empty("mode command");

    for command in [":mode Legion Workflows", "  :mode   LEGION   WORKFLOWS  "] {
        assert_eq!(
            shell.handle_command(command).expect("mode command parses"),
            Some(CommandDispatchIntent::SetProductMode {
                mode: DockMode::Automate,
            }),
            "canonical command {command:?} must not fall back to Manual"
        );
    }
}

#[test]
fn mode_command_dispatches_legacy_workflow_aliases_to_automate() {
    let mut shell = Shell::empty("mode aliases");

    for alias in [
        "automate",
        "automation",
        "autonomous",
        "LegionWorkflows",
        "workflow",
        "workflows",
        "w",
    ] {
        let command = format!(":mode {alias}");
        assert_eq!(
            shell.handle_command(&command).expect("mode command parses"),
            Some(CommandDispatchIntent::SetProductMode {
                mode: DockMode::Automate,
            }),
            "legacy alias {alias:?} must retain its Automate compatibility binding"
        );
    }
}

#[test]
fn terminal_command_help_uses_only_canonical_mode_labels() {
    let help = terminal_command_help();

    assert!(help.contains(":mode Manual|Assist|Delegate|Legion Workflows"));
    for legacy_label in ["Automate", "Autonomous", "Delegates"] {
        assert!(
            !help.contains(legacy_label),
            "help must not expose legacy mode label {legacy_label:?}"
        );
    }
}

#[test]
fn settings_projection_parses_labels_and_normalizes_bounds() {
    let settings = SettingsProjection {
        theme_preference: ThemePreferenceProjection::parse("System")
            .expect("theme label should parse"),
        zoom_percent: 999,
        editor_font_family: "  JetBrains Mono<script>\n".to_string(),
        editor_font_size_pt: 1,
        terminal_shell_selection: String::new(),
        font_fallback_diagnostics: (0..9)
            .map(|index| WorkbenchFontFallbackDiagnostic {
                requested_family_label: "JetBrains Mono".to_string(),
                resolved_family_label: "legion-cjk-fallback".to_string(),
                coverage_label: format!("cjk-{index}"),
                fallback_found: true,
                message: "CJK fallback loaded from host font catalog".to_string(),
                schema_version: 1,
            })
            .collect(),
        toast_verbosity: ToastVerbosityProjection::parse("All statuses")
            .expect("toast label should parse"),
        editor: EditorSettingsProjection {
            line_numbers_visible: false,
            current_line_highlight: false,
            sticky_headers_visible: true,
            code_folding_visible: true,
            minimap_visible: false,
            whitespace_guides_visible: false,
            indent_guides_visible: false,
            smooth_scrolling_enabled: true,
            line_wrapping_policy: LineWrappingPolicy::FixedColumn,
            wrap_column: Some(12),
        },
        telemetry: WorkbenchTelemetryConsent::default(),
        indexed_workspace_search_enabled: false,
        next_edit_prediction_enabled: false,
        schema_version: 0,
    }
    .normalized();

    assert_eq!(settings.theme_preference, ThemePreferenceProjection::System);
    assert_eq!(settings.zoom_percent, SettingsProjection::MAX_ZOOM_PERCENT);
    assert_eq!(settings.editor_font_family, "JetBrains Monoscript");
    assert_eq!(
        settings.editor_font_size_pt,
        SettingsProjection::MIN_EDITOR_FONT_SIZE_PT
    );
    assert_eq!(settings.font_fallback_diagnostics.len(), 8);
    assert_eq!(settings.toast_verbosity, ToastVerbosityProjection::All);
    assert_eq!(
        settings.editor.line_wrapping_policy,
        LineWrappingPolicy::FixedColumn
    );
    assert_eq!(settings.editor.wrap_column, Some(40));
    assert!(!settings.editor.line_numbers_visible);
    assert!(!settings.editor.current_line_highlight);
    assert!(!settings.telemetry.crash_reports_enabled);
    assert_eq!(settings.telemetry.consent_label, "local-only");
    assert_eq!(settings.schema_version, 1);
}

#[test]
fn panel_registry_constructs_from_dock_panel_contracts() {
    let diagnostics = DockPanelDescriptor::new(
        PanelId::Diagnostics,
        "Problems",
        "alert",
        DockSide::Bottom,
        false,
    );
    let assistant = DockPanelDescriptor::new(
        PanelId::Assistant,
        "Assistant",
        "spark",
        DockSide::Right,
        true,
    );
    let panels: [&dyn DockPanel; 2] = [&diagnostics, &assistant];

    let registry = PanelRegistry::from_dock_panels(panels);

    assert!(registry.is_visible_in(PanelId::Diagnostics, DockMode::Manual));
    assert!(!registry.is_visible_in(PanelId::Assistant, DockMode::Manual));
    assert!(registry.is_visible_in(PanelId::Assistant, DockMode::Assist));
}

#[test]
fn dock_layouts_are_mode_scoped_and_manual_layout_is_ai_free() {
    let registry = PanelRegistry::standard();
    let manual = DockLayout::standard(DockMode::Manual);
    let automate = DockLayout::standard(DockMode::Automate);

    for side in [DockSide::Left, DockSide::Right, DockSide::Bottom] {
        let visible = manual.visible_panel_ids(side, &registry);
        assert!(
            visible
                .iter()
                .all(|id| registry.is_visible_in(*id, DockMode::Manual)),
            "manual {side:?} layout exposed an AI panel: {visible:?}"
        );
    }

    assert!(
        automate
            .visible_panel_ids(DockSide::Right, &registry)
            .contains(&PanelId::AgentFleet)
    );
    assert_ne!(manual.right.pinned_default, automate.right.pinned_default);
}

// --- P1.F2.T1: Manual-mode panel filtering regression suite ---
//
// These tests are the construction-time guarantee that Manual mode cannot
// expose any AI / provider / cloud / worker / delegation / collaboration
// / hosted-telemetry surface. They are intentionally written against the
// projection structures (PanelCapability, PanelRegistry, DockLayout) rather
// than against hard-coded panel id lists, so adding a new AI panel in the
// future without updating the mode filter will fail these tests.

use ProductRuntimeSurface::{
    AssistedAi, Automation, CloudProvider, Collaboration as CollaborationSurface, DelegatedTask,
    HostedTelemetry, ManualIde, NetworkEgress, PluginManagement, PluginRuntime,
    RemoteWorkspace as RemoteSurface, WorkerRuntime,
};

/// Runtime surfaces that Manual mode MUST NOT expose under any panel.
const FORBIDDEN_MANUAL_SURFACES: &[ProductRuntimeSurface] = &[
    AssistedAi,
    CloudProvider,
    NetworkEgress,
    HostedTelemetry,
    DelegatedTask,
    WorkerRuntime,
    Automation,
    CollaborationSurface,
    RemoteSurface,
    PluginRuntime,
];

/// Panels that the Manual dock layout MUST NOT reference.
const FORBIDDEN_MANUAL_PANEL_IDS: &[PanelId] = &[
    PanelId::Assistant,
    PanelId::Delegation,
    PanelId::ApprovalQueue,
    PanelId::AgentFleet,
    PanelId::DecisionFeed,
    PanelId::AgentLogs,
    PanelId::Workflow,
    PanelId::Collaboration,
    PanelId::RemoteWorkspace,
];

#[test]
fn manual_mode_allows_exactly_manual_ide_and_plugin_management() {
    use ProductRuntimeSurface::{ManualIde, PluginManagement};
    let allowed = [
        ProductRuntimeSurface::ManualIde,
        ProductRuntimeSurface::PluginManagement,
    ];
    for surface in [
        ManualIde,
        PluginManagement,
        AssistedAi,
        CloudProvider,
        NetworkEgress,
        HostedTelemetry,
        DelegatedTask,
        WorkerRuntime,
        Automation,
        CollaborationSurface,
        RemoteSurface,
        PluginRuntime,
    ] {
        let expected = allowed.contains(&surface);
        let actual = product_mode_allows_runtime_surface(ProductMode::Manual, surface);
        assert_eq!(
            actual, expected,
            "Manual mode filter for {surface:?} drifted from the construction-time allow-list"
        );
    }
}

#[test]
fn manual_registry_visibility_matches_capability_allow_list() {
    let registry = PanelRegistry::standard();
    // Every forbidden surface in the standard registry must be hidden
    // from Manual mode by construction, regardless of panel id.
    for panel in registry.panels() {
        let visible_in_manual = registry.is_visible_in(panel.id, DockMode::Manual);
        let has_forbidden_capability = panel
            .capabilities
            .iter()
            .any(|capability| FORBIDDEN_MANUAL_SURFACES.contains(capability));
        assert!(
            !(visible_in_manual && has_forbidden_capability),
            "panel `{}` ({:?}) leaked into Manual mode despite capabilities {:?}",
            panel.id.as_str(),
            panel.title,
            panel.capabilities,
        );
        // Conversely, every panel whose only capabilities are ManualIde
        // (or empty, which defaults to ManualIde) must be visible in Manual.
        let only_manual_capable = panel
            .capabilities
            .iter()
            .all(|capability| matches!(capability, ManualIde | PluginManagement));
        assert_eq!(
            visible_in_manual,
            only_manual_capable,
            "panel `{}` ({:?}) visibility disagrees with its capability set {:?}",
            panel.id.as_str(),
            panel.title,
            panel.capabilities,
        );
    }
}

#[test]
fn manual_dock_layout_never_references_forbidden_panels() {
    let registry = PanelRegistry::standard();
    let manual = DockLayout::standard(DockMode::Manual);

    for side in [DockSide::Left, DockSide::Right, DockSide::Bottom] {
        for panel_id in manual.visible_panel_ids(side, &registry) {
            assert!(
                !FORBIDDEN_MANUAL_PANEL_IDS.contains(&panel_id),
                "Manual {side:?} layout exposed forbidden panel {panel_id:?}"
            );
            assert!(
                registry.is_visible_in(panel_id, DockMode::Manual),
                "Manual {side:?} layout referenced panel {panel_id:?} \
                     that is not constructible in Manual mode"
            );
        }
    }
}

#[test]
fn manual_visible_for_returns_only_ai_free_panels_and_nonempty() {
    let registry = PanelRegistry::standard();
    let visible: Vec<_> = registry
        .visible_for(DockMode::Manual)
        .into_iter()
        .map(|panel| panel.id)
        .collect();

    // Manual must still have a usable baseline of editor / workspace
    // surfaces — the filter is "hide AI chrome", not "hide everything".
    assert!(
        !visible.is_empty(),
        "Manual mode filtered out every panel; nothing left to render"
    );
    for required in [
        PanelId::ProjectExplorer,
        PanelId::Terminal,
        PanelId::Settings,
    ] {
        assert!(
            visible.contains(&required),
            "Manual mode is missing baseline panel {required:?}; visible={visible:?}"
        );
    }
    for forbidden in FORBIDDEN_MANUAL_PANEL_IDS {
        assert!(
            !visible.contains(forbidden),
            "Manual visible_for leaked forbidden panel {forbidden:?}; visible={visible:?}"
        );
    }
    // And the AI-flag must agree with the capability set, so no
    // requires_ai=true panel can sneak in.
    for panel in registry.visible_for(DockMode::Manual) {
        assert!(
            !panel.requires_ai,
            "panel `{}` ({:?}) has requires_ai=true but was visible in Manual",
            panel.id.as_str(),
            panel.title,
        );
    }
}

#[test]
fn manual_shell_projection_carries_no_forbidden_capability() {
    // Build the standard Manual shell projection snapshot. The Shell
    // itself is projection-only — this test asserts that the
    // construction pipeline cannot produce a Manual shell whose
    // dock-panel catalog references any AI/provider/cloud/worker
    // surface, treating the registry + layout as the contract surface
    // for "Manual mode chrome".
    let registry = PanelRegistry::standard();
    let layout = DockLayout::standard(DockMode::Manual);
    let all_visible: Vec<PanelId> = [DockSide::Left, DockSide::Right, DockSide::Bottom]
        .iter()
        .flat_map(|side| layout.visible_panel_ids(*side, &registry))
        .collect();

    for panel_id in &all_visible {
        let descriptor = registry
            .panel(*panel_id)
            .unwrap_or_else(|| panic!("layout referenced unknown panel {panel_id:?}"));
        for capability in &descriptor.capabilities {
            assert!(
                !FORBIDDEN_MANUAL_SURFACES.contains(capability),
                "Manual shell projection surface for panel `{}` carries \
                     forbidden capability {capability:?}; \
                     capabilities={:?}",
                descriptor.id.as_str(),
                descriptor.capabilities,
            );
        }
    }
}

fn test_coordinate(line: u32, character: u32) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(character as u64),
        utf16_offset: None,
    }
}

fn test_legion_workflow_projection() -> LegionWorkflowProjection {
    LegionWorkflowProjection {
        projection_id: "legion-workflow:test".to_string(),
        rows: vec![legion_protocol::LegionWorkflowProjectionRow {
            session_id: LegionWorkflowSessionId("session:legion:test".to_string()),
            directive_artifact_id: Some("artifact:directive:legion:test".to_string()),
            spec_artifact_id: Some("artifact:spec:legion:test".to_string()),
            task_graph_artifact_id: Some("artifact:task-graph:legion:test".to_string()),
            lifecycle_state: legion_protocol::LegionWorkflowState::WaitingForApproval,
            worker_count: 3,
            provider_route_required_count: 1,
            dependency_count: 2,
            unresolved_conflict_count: 1,
            verification_gate_count: 2,
            passed_verification_count: 1,
            sign_off_count: 2,
            signed_off_count: 1,
            linked_proposals: vec![ProposalId(42)],
            merge_readiness: legion_protocol::LegionWorkflowMergeReadiness {
                state: legion_protocol::LegionWorkflowMergeReadinessState::WaitingForApproval,
                blockers: vec![
                    legion_protocol::LegionWorkflowMergeReadinessBlocker::ApprovalRequired,
                ],
                labels: vec!["legion_workflow.waiting_for_approval".to_string()],
                redaction_hints: vec![RedactionHint::MetadataOnly],
                schema_version: 1,
            },
            display_safe_labels: vec![
                "implementer.local".to_string(),
                "Unattended merge unsupported until approval".to_string(),
            ],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        mcp_registries: Vec::new(),
        decision_feed: Vec::new(),
        risk_monitors: Vec::new(),
        kill_switches: Vec::new(),
        tool_permission_requests: Vec::new(),
        total_session_count: 1,
        mcp_registry_count: 0,
        decision_feed_count: 0,
        risk_monitor_count: 0,
        kill_switch_count: 0,
        tool_permission_request_count: 0,
        omitted_row_count: 0,
        generated_at: TimestampMillis(1),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn test_proposal_ledger_projection() -> ProposalLedgerProjection {
    ProposalLedgerProjection {
        rows: vec![ProposalLedgerRow {
            proposal_id: ProposalId(42),
            workspace_id: Some(WorkspaceId(1)),
            title: "bounded save preview".to_string(),
            payload_kind: ProposalPayloadKind::SaveFile,
            lifecycle: ProposalLifecycleStateDisplay {
                state: ProposalLifecycleState::Previewed,
                label: "Previewed".to_string(),
                description: "ready for user review".to_string(),
            },
            principal: PrincipalId("trusted".to_string()),
            capability: CapabilityId("fs.write".to_string()),
            created_at: TimestampMillis(1),
            updated_at: TimestampMillis(2),
            expires_at: None,
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            rollback: ProposalRollbackAvailability::Available,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: Vec::new(),
                omitted_target_count: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            context_manifest: ProposalContextManifestSummary {
                manifest_id: "manifest:42".to_string(),
                category_count: 1,
                total_item_count: 1,
                omitted_item_count: 0,
                categories: vec![ProposalContextManifestEntrySummary {
                    category: "files".to_string(),
                    item_count: 1,
                    omitted_item_count: 0,
                    privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                    manifest_hash: Some(FileFingerprint {
                        algorithm: "sha256".to_string(),
                        value: "ctx".to_string(),
                    }),
                    redaction_hints: vec![RedactionHint::MetadataOnly],
                }],
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            diff_summary: ProposalDiffSummary {
                kind: ProposalDiffSummaryKind::Text,
                target_count: 1,
                hunk_count: 1,
                inserted_line_count: 2,
                deleted_line_count: 1,
                omitted_hunk_count: 99,
                full_source_redacted: true,
                diff_hash: Some(FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "diff".to_string(),
                }),
                chunks: vec![ProposalDiffChunkDescriptor {
                    chunk_id: "chunk-0".to_string(),
                    target_id: None,
                    byte_range: Some(ByteRange::new(10, 20)),
                    changed_line_count: 3,
                    inserted_line_count: 2,
                    deleted_line_count: 1,
                    content_hash: Some(FileFingerprint {
                        algorithm: "blake3".to_string(),
                        value: "chunk".to_string(),
                    }),
                }],
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            preview_warnings: Vec::new(),
            diagnostics: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        selected_proposal_id: Some(ProposalId(42)),
        omitted_row_count: 0,
        generated_at: TimestampMillis(3),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn degraded_viewport_projection() -> ViewportProjection {
    ViewportProjection {
        workspace_id: WorkspaceId(1),
        buffer_id: BufferId(2),
        file_id: Some(FileId(9)),
        snapshot_id: SnapshotId(3),
        buffer_version: BufferVersion(4),
        visible_range: ProtocolTextRange {
            start: test_coordinate(10, 0),
            end: test_coordinate(12, 14),
        },
        selections: Vec::new(),
        cursor: test_coordinate(10, 0),
        cursors: Vec::new(),
        cursor_affinities: Vec::new(),
        scroll: ViewportScroll {
            top_line: 10,
            left_column: 0,
        },
        dimensions: ViewportDimensions {
            width_px: 800,
            height_px: 32,
        },
        line_wrapping_policy: LineWrappingPolicy::Off,
        wrap_column: None,
        mode: ViewportProjectionMode::DegradedLargeFile,
        line_slices: vec![
            ViewportLineSlice {
                line_number: 10,
                visible_text: "bounded-alpha".to_string(),
                byte_range: ByteRange::new(1024, 1037),
                utf16_range: Utf16Range {
                    start: Utf16Position {
                        line: 10,
                        character: 0,
                    },
                    end: Utf16Position {
                        line: 10,
                        character: 13,
                    },
                },
                chunk_hash: FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "chunk-a".to_string(),
                },
                truncation_state: ViewportLineTruncationState::None,
            },
            ViewportLineSlice {
                line_number: 11,
                visible_text: "bounded-beta".to_string(),
                byte_range: ByteRange::new(2048, 2060),
                utf16_range: Utf16Range {
                    start: Utf16Position {
                        line: 11,
                        character: 0,
                    },
                    end: Utf16Position {
                        line: 11,
                        character: 12,
                    },
                },
                chunk_hash: FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "chunk-b".to_string(),
                },
                truncation_state: ViewportLineTruncationState::Trailing,
            },
        ],
        line_metrics: vec![
            ViewportLineMetric {
                byte_length: 13,
                utf16_length: 13,
                line_start_byte_offset: None,
                line_start_utf16_offset: None,
                line_ending_width: 1,
                exact: true,
            },
            ViewportLineMetric {
                byte_length: 4096,
                utf16_length: 4096,
                line_start_byte_offset: None,
                line_start_utf16_offset: None,
                line_ending_width: 1,
                exact: true,
            },
        ],
        decoration_spans: Vec::new(),
        fold_ranges: Vec::new(),
        semantic_token_overlays: Vec::new(),
        large_file_status: Some(LargeFileStatus {
            threshold_bytes: 5 * 1024 * 1024,
            byte_len: 6 * 1024 * 1024,
            disabled_overlay_reasons: vec!["semantic token overlays deferred".to_string()],
            bounded_search_enabled: true,
            message: "Large file degraded mode: viewport payloads are chunked".to_string(),
        }),
        schema_version: 2,
    }
}

#[test]
fn shell_parses_commands_into_dispatch_intents_without_editor_ownership() {
    let mut shell = Shell::new(ShellProjectionSnapshot {
        product_mode: DockMode::Manual,
        layout_projection: ShellLayoutProjection::plain("t"),
        explorer_projection: ExplorerProjection {
            nodes: Vec::new(),
            selection: None,
        },
        active_buffer_projection: ActiveBufferProjection {
            workspace_id: Some(WorkspaceId(1)),
            buffer_id: Some(BufferId(2)),
            file_id: Some(FileId(9)),
            file_path: Some(CanonicalPath("a.md".to_string())),
            viewport: None,
            state: ActiveBufferProjectionState::Full,
            degraded: false,
            small_buffer_preview: Some("first".to_string()),
            dirty: false,
        },
        status_messages: Vec::new(),
        palette_projection: PaletteProjection::closed(),
        command_registry_projection: empty_command_registry_projection(),
        settings_projection: SettingsProjection::default(),
        proposal_ledger_projection: test_proposal_ledger_projection(),
        artifact_ledger_projection: empty_artifact_ledger_projection(),
        verification_run_projection: empty_verification_run_projection(),
        system_graph_projection: empty_system_graph_projection(),
        context_manifest_projection: empty_context_manifest_projection(),
        privacy_inspector_projection: empty_privacy_inspector_projection(),
        permission_budget_projection: empty_permission_budget_projection(),
        approval_checklist_projection: empty_approval_checklist_projection(),
        checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
        assisted_ai_projection: empty_assisted_ai_projection(),
        assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
        delegated_task_projection: empty_delegated_task_projection(),
        legion_workflow_projection: empty_legion_workflow_projection(),
        legion_workflow_board_columns: Vec::new(),
        legion_workflow_fleet_card_projections: Vec::new(),
        legion_workflow_comm_rows: Vec::new(),
        legion_workflow_budget_rows: Vec::new(),
        plugin_contribution_projections: Vec::new(),
        extension_catalog: Vec::new(),
        legion_cloud_lane: LegionCloudLaneProjection::disabled(),
        collaboration_presence_projections: Vec::new(),
        collaboration_gui_projection: CollaborationGuiProjection::disabled(),
        remote_gui_projection: RemoteGuiProjection::disabled(),
        daily_editing_projection: DailyEditingProjection::empty(),
        excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
        search_projection: SearchProjection::idle(),
        find_bar_projection: FindBarProjection::default(),
        structural_search_projection: StructuralSearchProjection::idle(),
        git_projection: GitProjection::idle(),
        debug_projection: DebugProjection::empty(),
        test_explorer_projection: TestExplorerProjection::empty(),
        language_tooling_projection: LanguageToolingProjection::empty(),
        terminal_panel_projection: TerminalPanelProjection::empty(),
    });

    let intent = shell
        .handle_command(":i \\n")
        .expect("insert command should parse")
        .expect("intent should be emitted");

    assert_eq!(
        intent,
        CommandDispatchIntent::Insert {
            buffer_id: BufferId(2),
            at: test_coordinate(0, 0),
            text: "\\n".to_string(),
        }
    );
    assert_eq!(
        shell.active_buffer_projection.small_buffer_text(),
        Some("first")
    );
    assert_eq!(shell.command_dispatch_intents.len(), 1);
}

#[test]
fn toast_stack_filters_info_bounds_visible_and_tracks_overflow() {
    let messages = (0..(TOAST_VISIBLE_LIMIT + 2))
        .map(|index| StatusMessageProjection {
            severity: StatusSeverity::Warning,
            message: format!("Warning {index}: detail"),
        })
        .chain(std::iter::once(StatusMessageProjection {
            severity: StatusSeverity::Info,
            message: "Info-only status".to_string(),
        }))
        .collect::<Vec<_>>();

    let stack = ToastStackProjection::from_status_messages(&messages, &[]);
    let all_stack = ToastStackProjection::from_status_messages_with_verbosity(
        &messages,
        &[],
        ToastVerbosityProjection::All,
    );
    let errors_only_stack = ToastStackProjection::from_status_messages_with_verbosity(
        &messages,
        &[],
        ToastVerbosityProjection::ErrorsOnly,
    );
    let dismissed = stack.visible[0].id;
    let dismissed_stack = ToastStackProjection::from_status_messages(&messages, &[dismissed]);

    assert_eq!(stack.visible.len(), TOAST_VISIBLE_LIMIT);
    assert_eq!(stack.overflow_count, 2);
    assert!(
        stack
            .visible
            .iter()
            .all(|toast| toast.severity != StatusSeverity::Info)
    );
    assert_eq!(stack.visible[0].title, "Warning 6");
    assert_eq!(stack.visible[0].body.as_deref(), Some("detail"));
    assert_eq!(all_stack.visible.len(), TOAST_VISIBLE_LIMIT);
    assert_eq!(all_stack.overflow_count, 3);
    assert_eq!(all_stack.visible[0].severity, StatusSeverity::Info);
    assert!(errors_only_stack.visible.is_empty());
    assert_eq!(errors_only_stack.overflow_count, 0);
    assert_eq!(dismissed_stack.visible.len(), TOAST_VISIBLE_LIMIT);
    assert_eq!(dismissed_stack.overflow_count, 1);
    assert!(
        dismissed_stack
            .visible
            .iter()
            .all(|toast| toast.id != dismissed)
    );
}

#[test]
fn shell_renders_proposal_ledger_from_static_snapshot() {
    let ledger = test_proposal_ledger_projection();
    let shell = Shell::new(ShellProjectionSnapshot {
        product_mode: DockMode::Manual,
        layout_projection: ShellLayoutProjection::plain("t"),
        explorer_projection: ExplorerProjection {
            nodes: Vec::new(),
            selection: None,
        },
        active_buffer_projection: ActiveBufferProjection::empty(),
        status_messages: Vec::new(),
        palette_projection: PaletteProjection::closed(),
        command_registry_projection: empty_command_registry_projection(),
        settings_projection: SettingsProjection::default(),
        proposal_ledger_projection: ledger.clone(),
        artifact_ledger_projection: empty_artifact_ledger_projection(),
        verification_run_projection: empty_verification_run_projection(),
        system_graph_projection: empty_system_graph_projection(),
        context_manifest_projection: empty_context_manifest_projection(),
        privacy_inspector_projection: empty_privacy_inspector_projection(),
        permission_budget_projection: empty_permission_budget_projection(),
        approval_checklist_projection: empty_approval_checklist_projection(),
        checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
        assisted_ai_projection: empty_assisted_ai_projection(),
        assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
        delegated_task_projection: empty_delegated_task_projection(),
        legion_workflow_projection: empty_legion_workflow_projection(),
        legion_workflow_board_columns: Vec::new(),
        legion_workflow_fleet_card_projections: Vec::new(),
        legion_workflow_comm_rows: Vec::new(),
        legion_workflow_budget_rows: Vec::new(),
        plugin_contribution_projections: Vec::new(),
        extension_catalog: Vec::new(),
        legion_cloud_lane: LegionCloudLaneProjection::disabled(),
        collaboration_presence_projections: Vec::new(),
        collaboration_gui_projection: CollaborationGuiProjection::disabled(),
        remote_gui_projection: RemoteGuiProjection::disabled(),
        daily_editing_projection: DailyEditingProjection::empty(),
        excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
        search_projection: SearchProjection::idle(),
        find_bar_projection: FindBarProjection::default(),
        structural_search_projection: StructuralSearchProjection::idle(),
        git_projection: GitProjection::idle(),
        debug_projection: DebugProjection::empty(),
        test_explorer_projection: TestExplorerProjection::empty(),
        language_tooling_projection: LanguageToolingProjection::empty(),
        terminal_panel_projection: TerminalPanelProjection::empty(),
    });

    let snapshot = shell.projection_snapshot();
    assert_eq!(snapshot.proposal_ledger_projection, ledger);
    assert_eq!(
        snapshot.proposal_ledger_projection.rows[0].proposal_id,
        ProposalId(42)
    );
    assert!(
        snapshot.proposal_ledger_projection.rows[0]
            .diff_summary
            .full_source_redacted
    );
}

#[test]
fn shell_carries_post_ga_work_surface_projections_without_ownership() {
    let mut snapshot = Shell::empty("work-surfaces").projection_snapshot();
    snapshot.command_registry_projection = legion_protocol::CommandRegistryProjection {
        projection_id: "command-registry:test".to_string(),
        commands: vec![legion_protocol::CommandDescriptor {
            command_id: "delegated.inspect_plan".to_string(),
            title: "Inspect Delegated Plan".to_string(),
            scope: "agents".to_string(),
            enabled: true,
            disabled_reason: None,
            shortcut: None,
            risk_label: legion_protocol::CommandRiskLabel::Safe,
            required_permission: Some(CapabilityId("delegated.plan.inspect".to_string())),
            target: Some("plan:1".to_string()),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        selected_command_id: None,
        omitted_command_count: 0,
        generated_at: TimestampMillis(1),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot.artifact_ledger_projection = legion_protocol::ArtifactLedgerProjection {
        projection_id: "artifact-ledger:test".to_string(),
        rows: vec![legion_protocol::ArtifactLedgerRow {
            artifact_id: "artifact:directive:1".to_string(),
            kind: legion_protocol::ArtifactKind::Directive,
            title: "Directive".to_string(),
            state_label: "Planned".to_string(),
            linked_proposal_id: None,
            linked_session_id: None,
            raw_payload_retained: false,
            risk_label: ProposalRiskLabel::Medium,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        omitted_row_count: 0,
        generated_at: TimestampMillis(1),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot.verification_run_projection = legion_protocol::VerificationRunProjection {
        projection_id: "verification-runs:test".to_string(),
        rows: vec![legion_protocol::VerificationRunRow {
            run_id: "verification:1".to_string(),
            label: "cargo test".to_string(),
            state: legion_protocol::VerificationRunState::Planned,
            command_class_label: "test".to_string(),
            command_body_redacted: true,
            exit_code: None,
            target_labels: vec!["workspace".to_string()],
            evidence_artifact_id: None,
            started_at: None,
            completed_at: None,
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        omitted_row_count: 0,
        generated_at: TimestampMillis(1),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot.system_graph_projection = legion_protocol::SystemGraphProjection {
        projection_id: "system-graph:test".to_string(),
        nodes: vec![legion_protocol::SystemGraphNode {
            node_id: "system:workspace".to_string(),
            kind_label: "workspace".to_string(),
            display_label: "Active workspace".to_string(),
            target_count: 1,
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        edges: Vec::new(),
        omitted_node_count: 0,
        omitted_edge_count: 0,
        generated_at: TimestampMillis(1),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };

    let shell = Shell::new(snapshot.clone());
    let roundtrip = shell.projection_snapshot();
    assert_eq!(
        roundtrip.command_registry_projection,
        snapshot.command_registry_projection
    );
    assert_eq!(
        roundtrip.artifact_ledger_projection,
        snapshot.artifact_ledger_projection
    );
    assert_eq!(
        roundtrip.verification_run_projection,
        snapshot.verification_run_projection
    );
    assert_eq!(
        roundtrip.system_graph_projection,
        snapshot.system_graph_projection
    );
    assert!(shell.command_dispatch_intents.is_empty());
}

#[test]
fn shell_snapshot_large_file_projection_carries_only_viewport_slices() {
    let large_source_len = 6 * 1024 * 1024;
    let shell = Shell::new(ShellProjectionSnapshot {
        product_mode: DockMode::Manual,
        layout_projection: ShellLayoutProjection::plain("large"),
        explorer_projection: ExplorerProjection {
            nodes: Vec::new(),
            selection: None,
        },
        active_buffer_projection: ActiveBufferProjection {
            workspace_id: Some(WorkspaceId(1)),
            buffer_id: Some(BufferId(2)),
            file_id: Some(FileId(9)),
            file_path: Some(CanonicalPath("large.txt".to_string())),
            viewport: Some(degraded_viewport_projection()),
            state: ActiveBufferProjectionState::Degraded,
            degraded: true,
            small_buffer_preview: None,
            dirty: false,
        },
        status_messages: Vec::new(),
        palette_projection: PaletteProjection::closed(),
        command_registry_projection: empty_command_registry_projection(),
        settings_projection: SettingsProjection::default(),
        proposal_ledger_projection: test_proposal_ledger_projection(),
        artifact_ledger_projection: empty_artifact_ledger_projection(),
        verification_run_projection: empty_verification_run_projection(),
        system_graph_projection: empty_system_graph_projection(),
        context_manifest_projection: empty_context_manifest_projection(),
        privacy_inspector_projection: empty_privacy_inspector_projection(),
        permission_budget_projection: empty_permission_budget_projection(),
        approval_checklist_projection: empty_approval_checklist_projection(),
        checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
        assisted_ai_projection: empty_assisted_ai_projection(),
        assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
        delegated_task_projection: empty_delegated_task_projection(),
        legion_workflow_projection: empty_legion_workflow_projection(),
        legion_workflow_board_columns: Vec::new(),
        legion_workflow_fleet_card_projections: Vec::new(),
        legion_workflow_comm_rows: Vec::new(),
        legion_workflow_budget_rows: Vec::new(),
        plugin_contribution_projections: Vec::new(),
        extension_catalog: Vec::new(),
        legion_cloud_lane: LegionCloudLaneProjection::disabled(),
        collaboration_presence_projections: Vec::new(),
        collaboration_gui_projection: CollaborationGuiProjection::disabled(),
        remote_gui_projection: RemoteGuiProjection::disabled(),
        daily_editing_projection: DailyEditingProjection::empty(),
        excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
        search_projection: SearchProjection::idle(),
        find_bar_projection: FindBarProjection::default(),
        structural_search_projection: StructuralSearchProjection::idle(),
        git_projection: GitProjection::idle(),
        debug_projection: DebugProjection::empty(),
        test_explorer_projection: TestExplorerProjection::empty(),
        language_tooling_projection: LanguageToolingProjection::empty(),
        terminal_panel_projection: TerminalPanelProjection::empty(),
    });

    let snapshot = shell.projection_snapshot();
    let active = snapshot.active_buffer_projection;
    let viewport = active.viewport.as_ref().expect("viewport projection");
    let payload_bytes = viewport
        .line_slices
        .iter()
        .map(|slice| slice.visible_text.len())
        .sum::<usize>();

    assert!(active.degraded);
    assert!(active.small_buffer_text().is_none());
    assert_eq!(viewport.mode, ViewportProjectionMode::DegradedLargeFile);
    assert!(viewport.large_file_status.is_some());
    assert!(payload_bytes < large_source_len / 1000);
    assert!(
        viewport
            .line_slices
            .iter()
            .all(|slice| slice.visible_text.len() < large_source_len)
    );
}

#[test]
fn shell_proposal_intents_do_not_mutate_editor_or_workspace_projection() {
    let mut shell = Shell::new(ShellProjectionSnapshot {
        product_mode: DockMode::Manual,
        layout_projection: ShellLayoutProjection::plain("t"),
        explorer_projection: ExplorerProjection {
            nodes: Vec::new(),
            selection: None,
        },
        active_buffer_projection: ActiveBufferProjection {
            workspace_id: Some(WorkspaceId(1)),
            buffer_id: Some(BufferId(2)),
            file_id: Some(FileId(9)),
            file_path: Some(CanonicalPath("a.md".to_string())),
            viewport: None,
            state: ActiveBufferProjectionState::Full,
            degraded: false,
            small_buffer_preview: Some("first".to_string()),
            dirty: false,
        },
        status_messages: Vec::new(),
        palette_projection: PaletteProjection::closed(),
        command_registry_projection: empty_command_registry_projection(),
        settings_projection: SettingsProjection::default(),
        proposal_ledger_projection: test_proposal_ledger_projection(),
        artifact_ledger_projection: empty_artifact_ledger_projection(),
        verification_run_projection: empty_verification_run_projection(),
        system_graph_projection: empty_system_graph_projection(),
        context_manifest_projection: empty_context_manifest_projection(),
        privacy_inspector_projection: empty_privacy_inspector_projection(),
        permission_budget_projection: empty_permission_budget_projection(),
        approval_checklist_projection: empty_approval_checklist_projection(),
        checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
        assisted_ai_projection: empty_assisted_ai_projection(),
        assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
        delegated_task_projection: empty_delegated_task_projection(),
        legion_workflow_projection: empty_legion_workflow_projection(),
        legion_workflow_board_columns: Vec::new(),
        legion_workflow_fleet_card_projections: Vec::new(),
        legion_workflow_comm_rows: Vec::new(),
        legion_workflow_budget_rows: Vec::new(),
        plugin_contribution_projections: Vec::new(),
        extension_catalog: Vec::new(),
        legion_cloud_lane: LegionCloudLaneProjection::disabled(),
        collaboration_presence_projections: Vec::new(),
        collaboration_gui_projection: CollaborationGuiProjection::disabled(),
        remote_gui_projection: RemoteGuiProjection::disabled(),
        daily_editing_projection: DailyEditingProjection::empty(),
        excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
        search_projection: SearchProjection::idle(),
        find_bar_projection: FindBarProjection::default(),
        structural_search_projection: StructuralSearchProjection::idle(),
        git_projection: GitProjection::idle(),
        debug_projection: DebugProjection::empty(),
        test_explorer_projection: TestExplorerProjection::empty(),
        language_tooling_projection: LanguageToolingProjection::empty(),
        terminal_panel_projection: TerminalPanelProjection::empty(),
    });

    let before = shell.projection_snapshot();
    let intent = shell
        .handle_command(":proposal-approve 42")
        .expect("proposal command should parse")
        .expect("intent should be emitted");

    assert_eq!(
        intent,
        CommandDispatchIntent::ApproveProposal {
            proposal_id: ProposalId(42)
        }
    );
    assert_eq!(shell.projection_snapshot(), before);
    assert_eq!(shell.command_dispatch_intents.len(), 1);
}

#[test]
fn control_trust_command_intents_remain_projection_only() {
    let mut shell = Shell::new(ShellProjectionSnapshot {
        product_mode: DockMode::Manual,
        layout_projection: ShellLayoutProjection::plain("control-trust"),
        explorer_projection: ExplorerProjection {
            nodes: Vec::new(),
            selection: None,
        },
        active_buffer_projection: ActiveBufferProjection {
            workspace_id: Some(WorkspaceId(1)),
            buffer_id: Some(BufferId(2)),
            file_id: Some(FileId(9)),
            file_path: Some(CanonicalPath("a.md".to_string())),
            viewport: None,
            state: ActiveBufferProjectionState::Full,
            degraded: false,
            small_buffer_preview: Some("first".to_string()),
            dirty: true,
        },
        status_messages: Vec::new(),
        palette_projection: PaletteProjection::closed(),
        command_registry_projection: empty_command_registry_projection(),
        settings_projection: SettingsProjection::default(),
        proposal_ledger_projection: test_proposal_ledger_projection(),
        artifact_ledger_projection: empty_artifact_ledger_projection(),
        verification_run_projection: empty_verification_run_projection(),
        system_graph_projection: empty_system_graph_projection(),
        context_manifest_projection: empty_context_manifest_projection(),
        privacy_inspector_projection: empty_privacy_inspector_projection(),
        permission_budget_projection: empty_permission_budget_projection(),
        approval_checklist_projection: empty_approval_checklist_projection(),
        checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
        assisted_ai_projection: empty_assisted_ai_projection(),
        assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
        delegated_task_projection: empty_delegated_task_projection(),
        legion_workflow_projection: empty_legion_workflow_projection(),
        legion_workflow_board_columns: Vec::new(),
        legion_workflow_fleet_card_projections: Vec::new(),
        legion_workflow_comm_rows: Vec::new(),
        legion_workflow_budget_rows: Vec::new(),
        plugin_contribution_projections: Vec::new(),
        extension_catalog: Vec::new(),
        legion_cloud_lane: LegionCloudLaneProjection::disabled(),
        collaboration_presence_projections: Vec::new(),
        collaboration_gui_projection: CollaborationGuiProjection::disabled(),
        remote_gui_projection: RemoteGuiProjection::disabled(),
        daily_editing_projection: DailyEditingProjection::empty(),
        excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
        search_projection: SearchProjection::idle(),
        find_bar_projection: FindBarProjection::default(),
        structural_search_projection: StructuralSearchProjection::idle(),
        git_projection: GitProjection::idle(),
        debug_projection: DebugProjection::empty(),
        test_explorer_projection: TestExplorerProjection::empty(),
        language_tooling_projection: LanguageToolingProjection::empty(),
        terminal_panel_projection: TerminalPanelProjection::empty(),
    });
    let before = shell.projection_snapshot();

    let commands = vec![
        (
            ":proposal-preview 42",
            CommandDispatchIntent::PreviewProposal {
                proposal_id: ProposalId(42),
            },
        ),
        (
            ":proposal-approve 42",
            CommandDispatchIntent::ApproveProposal {
                proposal_id: ProposalId(42),
            },
        ),
        (
            ":proposal-reject 42",
            CommandDispatchIntent::RejectProposal {
                proposal_id: ProposalId(42),
                reason: ProposalRejectionReason::UserRejected,
            },
        ),
        (
            ":proposal-apply 42",
            CommandDispatchIntent::ApplyProposal {
                proposal_id: ProposalId(42),
            },
        ),
        (
            ":proposal-rollback 42",
            CommandDispatchIntent::RollbackProposal {
                proposal_id: ProposalId(42),
                reason: ProposalRollbackReason::UserRequested,
            },
        ),
        (
            ":proposal-cancel 42",
            CommandDispatchIntent::CancelProposal {
                proposal_id: ProposalId(42),
                reason: ProposalCancellationReason::UserCancelled,
            },
        ),
        (
            ":proposal-details 42",
            CommandDispatchIntent::OpenProposalDetails {
                proposal_id: ProposalId(42),
            },
        ),
        (
            ":ai-start summarize context",
            CommandDispatchIntent::StartAiRun {
                instruction_label: "summarize context".to_string(),
            },
        ),
        (
            ":ai-explain summarize context",
            CommandDispatchIntent::StartAiExplain {
                instruction_label: "summarize context".to_string(),
            },
        ),
        (
            ":ai-propose add guard",
            CommandDispatchIntent::StartAiProposal {
                instruction_label: "add guard".to_string(),
                selection: None,
            },
        ),
        (
            ":delegate-chat explain impacted files",
            CommandDispatchIntent::SendDelegateChat {
                prompt_label: "explain impacted files".to_string(),
            },
        ),
        (
            ":delegate-hunk 42 delegate-hunk-1 accept",
            CommandDispatchIntent::ReviewDelegateProposalHunk {
                proposal_id: ProposalId(42),
                hunk_id: "delegate-hunk-1".to_string(),
                disposition: DelegatedTaskProposalHunkDisposition::Accepted,
            },
        ),
        (
            ":delegate-permission delegate-permission-1 always",
            CommandDispatchIntent::RecordDelegateToolPermission {
                request_id: "delegate-permission-1".to_string(),
                decision: DelegatedTaskToolPermissionDecision::Always,
            },
        ),
        (
            ":ai-cancel run-1",
            CommandDispatchIntent::CancelAiRun {
                run_id: AgentRunId("run-1".to_string()),
            },
        ),
        (
            ":ai-replay run-1",
            CommandDispatchIntent::ReplayAiRun {
                run_id: AgentRunId("run-1".to_string()),
            },
        ),
        (
            ":ai-inspect run-1",
            CommandDispatchIntent::InspectAiRun {
                run_id: AgentRunId("run-1".to_string()),
            },
        ),
    ];

    let command_count = commands.len();
    for (command, expected) in commands {
        let intent = shell
            .handle_command(command)
            .expect("control trust command should parse")
            .expect("intent should be emitted");
        assert_eq!(intent, expected);
        assert_eq!(shell.projection_snapshot(), before);
    }

    assert!(shell.command_dispatch_intents.len() >= command_count);
}

#[test]
fn assisted_ai_command_intents_remain_projection_only() {
    control_trust_command_intents_remain_projection_only();
}

#[test]
fn control_trust_shell_carries_static_projection_contracts_without_ownership() {
    shell_renders_context_manifest_from_static_snapshot_without_ownership();
    shell_renders_privacy_and_budget_summaries_from_static_snapshot_without_ownership();
    shell_renders_approval_and_rollback_summaries_from_static_snapshot_without_ownership();
    shell_renders_assisted_ai_projection_from_static_snapshot_without_ownership();
}

#[test]
fn shell_renders_context_manifest_from_static_snapshot_without_ownership() {
    let mut manifest = empty_context_manifest_projection();
    manifest.manifest.manifest_id = "manifest:trust-review".to_string();
    manifest.manifest.risk_label = ProposalRiskLabel::Medium;
    manifest.manifest.privacy_label = ProposalPrivacyLabel::WorkspaceMetadata;
    manifest.selected_item_id = Some("semantic-job:0".to_string());
    manifest
        .manifest
        .items
        .push(legion_protocol::ContextManifestItem {
            item_id: "semantic-job:0".to_string(),
            kind: legion_protocol::ContextManifestItemKind::SemanticFabricJob,
            inclusion: legion_protocol::ContextManifestInclusionState::Included,
            workspace_id: Some(WorkspaceId(1)),
            file_id: Some(FileId(9)),
            buffer_id: Some(BufferId(2)),
            proposal_id: Some(ProposalId(42)),
            target_id: Some("target-buffer-main".to_string()),
            path: Some(CanonicalPath("C:/repo/src/main.rs".to_string())),
            ranges: vec![ByteRange::new(10, 20)],
            counts: vec![legion_protocol::ContextManifestItemCount {
                label: "diagnostics".to_string(),
                count: 2,
            }],
            hashes: vec![FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "content".to_string(),
            }],
            privacy_scope: Some(legion_protocol::SemanticPrivacyScope::Workspace),
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: ProposalRiskLabel::Medium,
            egress: legion_protocol::ContextManifestEgressStatus::LocalOnly,
            freshness: None,
            preconditions: None,
            labels: vec!["semantic.fabric.metadata".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        });
    manifest
        .manifest
        .items
        .push(legion_protocol::ContextManifestItem {
            item_id: "lsp-diagnostics:0".to_string(),
            kind: legion_protocol::ContextManifestItemKind::LspDiagnosticSummary,
            inclusion: legion_protocol::ContextManifestInclusionState::Excluded,
            workspace_id: Some(WorkspaceId(1)),
            file_id: Some(FileId(10)),
            buffer_id: Some(BufferId(3)),
            proposal_id: Some(ProposalId(42)),
            target_id: Some("target-buffer-secondary".to_string()),
            path: Some(CanonicalPath("C:/repo/src/lib.rs".to_string())),
            ranges: Vec::new(),
            counts: Vec::new(),
            hashes: Vec::new(),
            privacy_scope: Some(legion_protocol::SemanticPrivacyScope::Workspace),
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: ProposalRiskLabel::Medium,
            egress: legion_protocol::ContextManifestEgressStatus::LocalOnly,
            freshness: None,
            preconditions: None,
            labels: vec!["retrieval.excluded".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        });

    let shell = Shell::new(ShellProjectionSnapshot {
        product_mode: DockMode::Manual,
        layout_projection: ShellLayoutProjection::plain("trust"),
        explorer_projection: ExplorerProjection {
            nodes: Vec::new(),
            selection: None,
        },
        active_buffer_projection: ActiveBufferProjection::empty(),
        status_messages: Vec::new(),
        palette_projection: PaletteProjection::closed(),
        command_registry_projection: empty_command_registry_projection(),
        settings_projection: SettingsProjection::default(),
        proposal_ledger_projection: test_proposal_ledger_projection(),
        artifact_ledger_projection: empty_artifact_ledger_projection(),
        verification_run_projection: empty_verification_run_projection(),
        system_graph_projection: empty_system_graph_projection(),
        context_manifest_projection: manifest.clone(),
        privacy_inspector_projection: empty_privacy_inspector_projection(),
        permission_budget_projection: empty_permission_budget_projection(),
        approval_checklist_projection: empty_approval_checklist_projection(),
        checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
        assisted_ai_projection: empty_assisted_ai_projection(),
        assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
        delegated_task_projection: empty_delegated_task_projection(),
        legion_workflow_projection: empty_legion_workflow_projection(),
        legion_workflow_board_columns: Vec::new(),
        legion_workflow_fleet_card_projections: Vec::new(),
        legion_workflow_comm_rows: Vec::new(),
        legion_workflow_budget_rows: Vec::new(),
        plugin_contribution_projections: Vec::new(),
        extension_catalog: Vec::new(),
        legion_cloud_lane: LegionCloudLaneProjection::disabled(),
        collaboration_presence_projections: Vec::new(),
        collaboration_gui_projection: CollaborationGuiProjection::disabled(),
        remote_gui_projection: RemoteGuiProjection::disabled(),
        daily_editing_projection: DailyEditingProjection::empty(),
        excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
        search_projection: SearchProjection::idle(),
        find_bar_projection: FindBarProjection::default(),
        structural_search_projection: StructuralSearchProjection::idle(),
        git_projection: GitProjection::idle(),
        debug_projection: DebugProjection::empty(),
        test_explorer_projection: TestExplorerProjection::empty(),
        language_tooling_projection: LanguageToolingProjection::empty(),
        terminal_panel_projection: TerminalPanelProjection::empty(),
    });

    let snapshot = shell.projection_snapshot();
    assert_eq!(snapshot.context_manifest_projection, manifest);
    assert_eq!(snapshot.context_manifest_projection.manifest.items.len(), 2);
    assert_eq!(
        snapshot
            .context_manifest_projection
            .selected_item_id
            .as_deref(),
        Some("semantic-job:0")
    );
    assert_eq!(
        snapshot.context_manifest_projection.manifest.items[1].inclusion,
        legion_protocol::ContextManifestInclusionState::Excluded
    );
    assert!(shell.command_dispatch_intents.is_empty());
}

#[test]
fn context_manifest_selection_commands_remain_projection_only() {
    let mut manifest = empty_context_manifest_projection();
    manifest
        .manifest
        .items
        .push(legion_protocol::ContextManifestItem {
            item_id: "semantic-job:0".to_string(),
            kind: legion_protocol::ContextManifestItemKind::SemanticFabricJob,
            inclusion: legion_protocol::ContextManifestInclusionState::Included,
            workspace_id: Some(WorkspaceId(1)),
            file_id: Some(FileId(9)),
            buffer_id: Some(BufferId(2)),
            proposal_id: Some(ProposalId(42)),
            target_id: Some("target-buffer-main".to_string()),
            path: Some(CanonicalPath("C:/repo/src/main.rs".to_string())),
            ranges: vec![ByteRange::new(10, 20)],
            counts: vec![legion_protocol::ContextManifestItemCount {
                label: "diagnostics".to_string(),
                count: 2,
            }],
            hashes: vec![FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "content".to_string(),
            }],
            privacy_scope: Some(legion_protocol::SemanticPrivacyScope::Workspace),
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            risk_label: ProposalRiskLabel::Medium,
            egress: legion_protocol::ContextManifestEgressStatus::LocalOnly,
            freshness: None,
            preconditions: None,
            labels: vec!["semantic.fabric.metadata".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        });

    let mut shell = Shell::new(ShellProjectionSnapshot {
        product_mode: DockMode::Manual,
        layout_projection: ShellLayoutProjection::plain("trust"),
        explorer_projection: ExplorerProjection {
            nodes: Vec::new(),
            selection: None,
        },
        active_buffer_projection: ActiveBufferProjection::empty(),
        status_messages: Vec::new(),
        palette_projection: PaletteProjection::closed(),
        command_registry_projection: empty_command_registry_projection(),
        settings_projection: SettingsProjection::default(),
        proposal_ledger_projection: test_proposal_ledger_projection(),
        artifact_ledger_projection: empty_artifact_ledger_projection(),
        verification_run_projection: empty_verification_run_projection(),
        system_graph_projection: empty_system_graph_projection(),
        context_manifest_projection: manifest,
        privacy_inspector_projection: empty_privacy_inspector_projection(),
        permission_budget_projection: empty_permission_budget_projection(),
        approval_checklist_projection: empty_approval_checklist_projection(),
        checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
        assisted_ai_projection: empty_assisted_ai_projection(),
        assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
        delegated_task_projection: empty_delegated_task_projection(),
        legion_workflow_projection: empty_legion_workflow_projection(),
        legion_workflow_board_columns: Vec::new(),
        legion_workflow_fleet_card_projections: Vec::new(),
        legion_workflow_comm_rows: Vec::new(),
        legion_workflow_budget_rows: Vec::new(),
        plugin_contribution_projections: Vec::new(),
        extension_catalog: Vec::new(),
        legion_cloud_lane: LegionCloudLaneProjection::disabled(),
        collaboration_presence_projections: Vec::new(),
        collaboration_gui_projection: CollaborationGuiProjection::disabled(),
        remote_gui_projection: RemoteGuiProjection::disabled(),
        daily_editing_projection: DailyEditingProjection::empty(),
        excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
        search_projection: SearchProjection::idle(),
        find_bar_projection: FindBarProjection::default(),
        structural_search_projection: StructuralSearchProjection::idle(),
        git_projection: GitProjection::idle(),
        debug_projection: DebugProjection::empty(),
        test_explorer_projection: TestExplorerProjection::empty(),
        language_tooling_projection: LanguageToolingProjection::empty(),
        terminal_panel_projection: TerminalPanelProjection::empty(),
    });

    assert!(shell.command_dispatch_intents.is_empty());
    assert!(
        shell
            .handle_command(":context-manifest-select semantic-job:0")
            .expect("context manifest select should parse")
            .is_none()
    );
    assert_eq!(
        shell
            .projection_snapshot()
            .context_manifest_projection
            .selected_item_id
            .as_deref(),
        Some("semantic-job:0")
    );
    assert!(
        shell
            .handle_command(":context-manifest-clear")
            .expect("context manifest clear should parse")
            .is_none()
    );
    assert_eq!(
        shell
            .projection_snapshot()
            .context_manifest_projection
            .selected_item_id,
        None
    );
    assert!(shell.command_dispatch_intents.is_empty());
}

#[test]
fn shell_renders_privacy_and_budget_summaries_from_static_snapshot_without_ownership() {
    let mut privacy = empty_privacy_inspector_projection();
    privacy.inspector_id = "privacy:trust".to_string();
    privacy.records = vec![legion_protocol::PrivacyInspectorExposureRecord {
        exposure_id: "exposure:semantic".to_string(),
        source_kind: legion_protocol::PrivacyInspectorSourceKind::SemanticMetadata,
        context_item_id: Some("semantic:0".to_string()),
        proposal_id: Some(ProposalId(42)),
        target_id: Some("target-0".to_string()),
        workspace_id: Some(WorkspaceId(1)),
        file_id: Some(FileId(9)),
        buffer_id: Some(BufferId(2)),
        privacy_scope: Some(legion_protocol::SemanticPrivacyScope::Workspace),
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_state: legion_protocol::PrivacyInspectorRedactionState::MetadataOnly,
        inclusion: legion_protocol::ContextManifestInclusionState::Included,
        egress: legion_protocol::ContextManifestEgressStatus::LocalOnly,
        risk_label: ProposalRiskLabel::Low,
        permission_label: Some(CapabilityId("semantic.read".to_string())),
        ranges: vec![ByteRange::new(10, 20)],
        counts: Vec::new(),
        hashes: vec![FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "metadata-hash".to_string(),
        }],
        labels: vec!["semantic.metadata".to_string()],
        reasons: vec!["context.included".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];

    let mut budgets = empty_permission_budget_projection();
    budgets.projection_id = "budgets:trust".to_string();
    budgets.budgets = vec![PermissionBudgetContract {
        budget_id: "budget:semantic".to_string(),
        action_class: PermissionBudgetActionClass::ReadSemanticMetadata,
        capability: Some(CapabilityId("semantic.read".to_string())),
        state: PermissionBudgetState::Allowed,
        privacy_scope: legion_protocol::SemanticPrivacyScope::MetadataOnly,
        usage: PermissionBudgetUsageSummary {
            unit_label: "items".to_string(),
            used: 1,
            ceiling: Some(10),
            remaining: Some(9),
            attempted: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        reset_policy_label: PermissionBudgetResetPolicyLabel::Session,
        consent_requirement_label: PermissionBudgetConsentRequirementLabel::NotRequired,
        risk_label: ProposalRiskLabel::Low,
        reasons: vec!["budget.seeded".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];

    let shell = Shell::new(ShellProjectionSnapshot {
        product_mode: DockMode::Manual,
        layout_projection: ShellLayoutProjection::plain("trust"),
        explorer_projection: ExplorerProjection {
            nodes: Vec::new(),
            selection: None,
        },
        active_buffer_projection: ActiveBufferProjection::empty(),
        status_messages: Vec::new(),
        palette_projection: PaletteProjection::closed(),
        command_registry_projection: empty_command_registry_projection(),
        settings_projection: SettingsProjection::default(),
        proposal_ledger_projection: test_proposal_ledger_projection(),
        artifact_ledger_projection: empty_artifact_ledger_projection(),
        verification_run_projection: empty_verification_run_projection(),
        system_graph_projection: empty_system_graph_projection(),
        context_manifest_projection: empty_context_manifest_projection(),
        privacy_inspector_projection: privacy.clone(),
        permission_budget_projection: budgets.clone(),
        approval_checklist_projection: empty_approval_checklist_projection(),
        checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
        assisted_ai_projection: empty_assisted_ai_projection(),
        assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
        delegated_task_projection: empty_delegated_task_projection(),
        legion_workflow_projection: empty_legion_workflow_projection(),
        legion_workflow_board_columns: Vec::new(),
        legion_workflow_fleet_card_projections: Vec::new(),
        legion_workflow_comm_rows: Vec::new(),
        legion_workflow_budget_rows: Vec::new(),
        plugin_contribution_projections: Vec::new(),
        extension_catalog: Vec::new(),
        legion_cloud_lane: LegionCloudLaneProjection::disabled(),
        collaboration_presence_projections: Vec::new(),
        collaboration_gui_projection: CollaborationGuiProjection::disabled(),
        remote_gui_projection: RemoteGuiProjection::disabled(),
        daily_editing_projection: DailyEditingProjection::empty(),
        excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
        search_projection: SearchProjection::idle(),
        find_bar_projection: FindBarProjection::default(),
        structural_search_projection: StructuralSearchProjection::idle(),
        git_projection: GitProjection::idle(),
        debug_projection: DebugProjection::empty(),
        test_explorer_projection: TestExplorerProjection::empty(),
        language_tooling_projection: LanguageToolingProjection::empty(),
        terminal_panel_projection: TerminalPanelProjection::empty(),
    });

    let snapshot = shell.projection_snapshot();
    assert_eq!(snapshot.privacy_inspector_projection, privacy);
    assert_eq!(snapshot.permission_budget_projection, budgets);
    assert!(shell.command_dispatch_intents.is_empty());
}

#[test]
fn shell_renders_approval_and_rollback_summaries_from_static_snapshot_without_ownership() {
    let mut checklist = empty_approval_checklist_projection();
    checklist.checklist_id = "approval-checklist:42".to_string();
    checklist.proposal_id = ProposalId(42);
    checklist.ready_for_approval = true;
    checklist.gates = vec![legion_protocol::ApprovalChecklistGateSummary {
        gate: legion_protocol::ApprovalChecklistGateKind::AuditBeforeSuccess,
        status: legion_protocol::ApprovalChecklistGateStatus::Satisfied,
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        labels: vec!["audit.metadata_only".to_string()],
        reasons: Vec::new(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];

    let mut rollback = empty_checkpoint_rollback_projection();
    rollback.projection_id = "checkpoint-rollback:42".to_string();
    rollback.proposal_id = ProposalId(42);
    rollback.checkpoint.available = true;
    rollback.rollback.availability = legion_protocol::ProposalRollbackAvailability::Available;
    rollback.targets = vec![legion_protocol::CheckpointRollbackTargetSummary {
        target_id: "target-buffer-main".to_string(),
        kind: legion_protocol::ProposalTargetKind::OpenBuffer,
        workspace_id: Some(WorkspaceId(1)),
        file_id: Some(FileId(9)),
        buffer_id: Some(BufferId(2)),
        terminal_session_id: None,
        plugin_id: None,
        ranges: vec![ByteRange::new(10, 20)],
        hashes: vec![FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "expected".to_string(),
        }],
        expected_file_content_version: Some(legion_protocol::FileContentVersion(44)),
        expected_buffer_version: Some(BufferVersion(55)),
        expected_snapshot_id: Some(SnapshotId(66)),
        expected_workspace_generation: Some(legion_protocol::WorkspaceGeneration(77)),
        labels: vec!["target.kind.OpenBuffer".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];

    let shell = Shell::new(ShellProjectionSnapshot {
        product_mode: DockMode::Manual,
        layout_projection: ShellLayoutProjection::plain("trust"),
        explorer_projection: ExplorerProjection {
            nodes: Vec::new(),
            selection: None,
        },
        active_buffer_projection: ActiveBufferProjection::empty(),
        status_messages: Vec::new(),
        palette_projection: PaletteProjection::closed(),
        command_registry_projection: empty_command_registry_projection(),
        settings_projection: SettingsProjection::default(),
        proposal_ledger_projection: test_proposal_ledger_projection(),
        artifact_ledger_projection: empty_artifact_ledger_projection(),
        verification_run_projection: empty_verification_run_projection(),
        system_graph_projection: empty_system_graph_projection(),
        context_manifest_projection: empty_context_manifest_projection(),
        privacy_inspector_projection: empty_privacy_inspector_projection(),
        permission_budget_projection: empty_permission_budget_projection(),
        approval_checklist_projection: checklist.clone(),
        checkpoint_rollback_projection: rollback.clone(),
        assisted_ai_projection: empty_assisted_ai_projection(),
        assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
        delegated_task_projection: empty_delegated_task_projection(),
        legion_workflow_projection: empty_legion_workflow_projection(),
        legion_workflow_board_columns: Vec::new(),
        legion_workflow_fleet_card_projections: Vec::new(),
        legion_workflow_comm_rows: Vec::new(),
        legion_workflow_budget_rows: Vec::new(),
        plugin_contribution_projections: Vec::new(),
        extension_catalog: Vec::new(),
        legion_cloud_lane: LegionCloudLaneProjection::disabled(),
        collaboration_presence_projections: Vec::new(),
        collaboration_gui_projection: CollaborationGuiProjection::disabled(),
        remote_gui_projection: RemoteGuiProjection::disabled(),
        daily_editing_projection: DailyEditingProjection::empty(),
        excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
        search_projection: SearchProjection::idle(),
        find_bar_projection: FindBarProjection::default(),
        structural_search_projection: StructuralSearchProjection::idle(),
        git_projection: GitProjection::idle(),
        debug_projection: DebugProjection::empty(),
        test_explorer_projection: TestExplorerProjection::empty(),
        language_tooling_projection: LanguageToolingProjection::empty(),
        terminal_panel_projection: TerminalPanelProjection::empty(),
    });

    let snapshot = shell.projection_snapshot();
    assert_eq!(snapshot.approval_checklist_projection, checklist);
    assert_eq!(snapshot.checkpoint_rollback_projection, rollback);
    assert!(snapshot.approval_checklist_projection.ready_for_approval);
    assert!(shell.command_dispatch_intents.is_empty());
}

#[test]
fn shell_renders_assisted_ai_projection_from_static_snapshot_without_ownership() {
    let mut assisted = empty_assisted_ai_projection();
    assisted.projection_id = "assisted-ai:p6-2".to_string();
    assisted.provider_count = 1;
    assisted.request_count = 1;
    assisted.preview_ready_count = 1;
    assisted.providers = vec![legion_protocol::AssistedAiProviderCapabilitySummary {
        provider_id: "provider:local-redacted".to_string(),
        provider_label: "Local metadata provider".to_string(),
        provider_class: legion_protocol::AssistedAiProviderClass::Local,
        supported_operations: vec![legion_protocol::AssistedAiOperationClass::ProposeEdit],
        supported_operation_count: 1,
        model_capability_label_count: 1,
        tool_capability_label_count: 0,
        context_window_label: "bounded".to_string(),
        cost_budget_label: "capped".to_string(),
        risk_budget_label: "review-required".to_string(),
        privacy_retention_label: "metadata-only".to_string(),
        availability: legion_protocol::AssistedAiProviderAvailabilityState::Available,
        refusal: None,
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];
    assisted.proposal_previews = vec![legion_protocol::AssistedAiProposalPreviewSummary {
        preview_id: "assist:preview:42".to_string(),
        output_id: "assist:output:42".to_string(),
        request_id: "assist:req:42".to_string(),
        provider_id: "provider:local-redacted".to_string(),
        proposal_id: ProposalId(42),
        payload_kind: ProposalPayloadKind::TextEdit,
        lifecycle_state: ProposalLifecycleState::Previewed,
        readiness: legion_protocol::AssistedAiProposalPreviewReadiness::PreviewReady,
        ready_for_preview: true,
        ready_for_approval: true,
        ready_for_apply: false,
        correlation_id: legion_protocol::CorrelationId(901),
        causality_id: legion_protocol::CausalityId(
            uuid::Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap(),
        ),
        context_manifest: legion_protocol::AssistedAiTrustProjectionReference {
            reference_id: "manifest:p5:context".to_string(),
            kind: legion_protocol::AssistedAiTrustProjectionKind::ContextManifest,
            projection_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "manifest".to_string(),
            },
            schema_version: 1,
        },
        approval_checklist: legion_protocol::AssistedAiTrustProjectionReference {
            reference_id: "checklist:p5:approval".to_string(),
            kind: legion_protocol::AssistedAiTrustProjectionKind::ProposalApprovalChecklist,
            projection_hash: FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "checklist".to_string(),
            },
            schema_version: 1,
        },
        checkpoint_rollback: None,
        preconditions: legion_protocol::ContextManifestPreconditionSummary::from_preconditions(
            &legion_protocol::ProposalVersionPreconditions {
                file_version: Some(legion_protocol::FileContentVersion(44)),
                buffer_version: Some(BufferVersion(55)),
                snapshot_id: Some(SnapshotId(66)),
                generation: Some(legion_protocol::WorkspaceGeneration(77)),
                file_content_version: Some(legion_protocol::FileContentVersion(44)),
                workspace_generation: Some(legion_protocol::WorkspaceGeneration(77)),
                expected_fingerprint: Some(FileFingerprint {
                    algorithm: "sha256".to_string(),
                    value: "expected".to_string(),
                }),
                expected_file_length: Some(1234),
                expected_modified_at: Some(TimestampMillis(9876)),
            },
            1,
        ),
        target_coverage: ProposalTargetCoverage {
            coverage_kind: ProposalTargetCoverageKind::Complete,
            targets: Vec::new(),
            omitted_target_count: 0,
            redaction_hints: vec![RedactionHint::MetadataOnly],
        },
        diff_summary: ProposalDiffSummary {
            kind: ProposalDiffSummaryKind::Text,
            target_count: 1,
            hunk_count: 1,
            inserted_line_count: 0,
            deleted_line_count: 0,
            omitted_hunk_count: 0,
            full_source_redacted: true,
            diff_hash: None,
            chunks: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
        },
        trust_projection_references: Vec::new(),
        ledger_row_present: true,
        preview_warning_count: 0,
        refusal: None,
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        labels: vec!["proposal.apply.not_encoded".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];

    let shell = Shell::new(ShellProjectionSnapshot {
        product_mode: DockMode::Manual,
        layout_projection: ShellLayoutProjection::plain("assisted"),
        explorer_projection: ExplorerProjection {
            nodes: Vec::new(),
            selection: None,
        },
        active_buffer_projection: ActiveBufferProjection::empty(),
        status_messages: Vec::new(),
        palette_projection: PaletteProjection::closed(),
        command_registry_projection: empty_command_registry_projection(),
        settings_projection: SettingsProjection::default(),
        proposal_ledger_projection: test_proposal_ledger_projection(),
        artifact_ledger_projection: empty_artifact_ledger_projection(),
        verification_run_projection: empty_verification_run_projection(),
        system_graph_projection: empty_system_graph_projection(),
        context_manifest_projection: empty_context_manifest_projection(),
        privacy_inspector_projection: empty_privacy_inspector_projection(),
        permission_budget_projection: empty_permission_budget_projection(),
        approval_checklist_projection: empty_approval_checklist_projection(),
        checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
        assisted_ai_projection: assisted.clone(),
        assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
        delegated_task_projection: empty_delegated_task_projection(),
        legion_workflow_projection: empty_legion_workflow_projection(),
        legion_workflow_board_columns: Vec::new(),
        legion_workflow_fleet_card_projections: Vec::new(),
        legion_workflow_comm_rows: Vec::new(),
        legion_workflow_budget_rows: Vec::new(),
        plugin_contribution_projections: Vec::new(),
        extension_catalog: Vec::new(),
        legion_cloud_lane: LegionCloudLaneProjection::disabled(),
        collaboration_presence_projections: Vec::new(),
        collaboration_gui_projection: CollaborationGuiProjection::disabled(),
        remote_gui_projection: RemoteGuiProjection::disabled(),
        daily_editing_projection: DailyEditingProjection::empty(),
        excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
        search_projection: SearchProjection::idle(),
        find_bar_projection: FindBarProjection::default(),
        structural_search_projection: StructuralSearchProjection::idle(),
        git_projection: GitProjection::idle(),
        debug_projection: DebugProjection::empty(),
        test_explorer_projection: TestExplorerProjection::empty(),
        language_tooling_projection: LanguageToolingProjection::empty(),
        terminal_panel_projection: TerminalPanelProjection::empty(),
    });

    let snapshot = shell.projection_snapshot();
    assert_eq!(snapshot.assisted_ai_projection, assisted);
    assert_eq!(
        snapshot.assisted_ai_projection.provider_invocation,
        legion_protocol::AssistedAiProviderInvocationState::NotEncoded
    );
    assert!(snapshot.assisted_ai_projection.proposal_previews[0].ready_for_preview);
    assert!(!snapshot.assisted_ai_projection.proposal_previews[0].ready_for_apply);
    assert!(shell.command_dispatch_intents.is_empty());
}

#[test]
fn shell_renders_delegated_task_projection_from_static_snapshot_without_ownership() {
    let mut delegated = empty_delegated_task_projection();
    delegated.projection_id = "delegated-task:p7-1".to_string();
    delegated.plan_count = 1;
    delegated.plan_rows = vec![legion_protocol::DelegatedTaskPlanRow {
        plan_id: legion_protocol::DelegatedTaskPlanId("plan:p7-1".to_string()),
        workspace_id: Some(WorkspaceId(1)),
        objective_summary_hash: FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "objective".to_string(),
        },
        plan_state: legion_protocol::DelegatedTaskPlanState::AwaitingApproval,
        readiness: legion_protocol::DelegatedTaskPlanReadinessStatus::PlanReady,
        step_count: 1,
        affected_target_count: 1,
        blocker_count: 0,
        refusal_count: 0,
        proposal_preview_link_count: 1,
        risk_label: ProposalRiskLabel::Medium,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        correlation_id: legion_protocol::CorrelationId(901),
        causality_id: legion_protocol::CausalityId(
            uuid::Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap(),
        ),
        runtime_activation: legion_protocol::DelegatedTaskRuntimeActivationState::NotEncoded,
        labels: vec!["delegated_task.plan_row.metadata_only".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];
    delegated.step_summaries = vec![legion_protocol::DelegatedTaskStepSummary {
        step_id: legion_protocol::DelegatedTaskStepId("step:preview".to_string()),
        plan_id: legion_protocol::DelegatedTaskPlanId("plan:p7-1".to_string()),
        order: 1,
        objective_summary_hash: FileFingerprint {
            algorithm: "sha256".to_string(),
            value: "step".to_string(),
        },
        operation_class: legion_protocol::DelegatedTaskOperationClass::LinkProposalPreview,
        state: legion_protocol::DelegatedTaskStepState::ProposalPreviewLinked,
        dependency_count: 0,
        target_count: 1,
        proposal_id: Some(ProposalId(42)),
        blocker_count: 0,
        risk_label: ProposalRiskLabel::Medium,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        labels: vec!["proposal-preview-link-only".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];

    let shell = Shell::new(ShellProjectionSnapshot {
        product_mode: DockMode::Manual,
        layout_projection: ShellLayoutProjection::plain("delegated"),
        explorer_projection: ExplorerProjection {
            nodes: Vec::new(),
            selection: None,
        },
        active_buffer_projection: ActiveBufferProjection::empty(),
        status_messages: Vec::new(),
        palette_projection: PaletteProjection::closed(),
        command_registry_projection: empty_command_registry_projection(),
        settings_projection: SettingsProjection::default(),
        proposal_ledger_projection: test_proposal_ledger_projection(),
        artifact_ledger_projection: empty_artifact_ledger_projection(),
        verification_run_projection: empty_verification_run_projection(),
        system_graph_projection: empty_system_graph_projection(),
        context_manifest_projection: empty_context_manifest_projection(),
        privacy_inspector_projection: empty_privacy_inspector_projection(),
        permission_budget_projection: empty_permission_budget_projection(),
        approval_checklist_projection: empty_approval_checklist_projection(),
        checkpoint_rollback_projection: empty_checkpoint_rollback_projection(),
        assisted_ai_projection: empty_assisted_ai_projection(),
        assist_inline_prediction_projection: AssistInlinePredictionProjection::empty(),
        delegated_task_projection: delegated.clone(),
        legion_workflow_projection: empty_legion_workflow_projection(),
        legion_workflow_board_columns: Vec::new(),
        legion_workflow_fleet_card_projections: Vec::new(),
        legion_workflow_comm_rows: Vec::new(),
        legion_workflow_budget_rows: Vec::new(),
        plugin_contribution_projections: Vec::new(),
        extension_catalog: Vec::new(),
        legion_cloud_lane: LegionCloudLaneProjection::disabled(),
        collaboration_presence_projections: Vec::new(),
        collaboration_gui_projection: CollaborationGuiProjection::disabled(),
        remote_gui_projection: RemoteGuiProjection::disabled(),
        daily_editing_projection: DailyEditingProjection::empty(),
        excerpt_surface_projection: ExcerptSurfaceProjection::empty(),
        search_projection: SearchProjection::idle(),
        find_bar_projection: FindBarProjection::default(),
        structural_search_projection: StructuralSearchProjection::idle(),
        git_projection: GitProjection::idle(),
        debug_projection: DebugProjection::empty(),
        test_explorer_projection: TestExplorerProjection::empty(),
        language_tooling_projection: LanguageToolingProjection::empty(),
        terminal_panel_projection: TerminalPanelProjection::empty(),
    });

    let snapshot = shell.projection_snapshot();
    assert_eq!(snapshot.delegated_task_projection, delegated);
    assert_eq!(
        snapshot.delegated_task_projection.runtime_activation,
        legion_protocol::DelegatedTaskRuntimeActivationState::NotEncoded
    );
    assert_eq!(
        snapshot.delegated_task_projection.step_summaries[0].proposal_id,
        Some(ProposalId(42))
    );
    assert!(shell.command_dispatch_intents.is_empty());
}

#[test]
fn legion_workflow_empty_projection_is_metadata_only() {
    let shell = Shell::empty("legion");
    let snapshot = shell.projection_snapshot();

    assert!(snapshot.legion_workflow_projection.rows.is_empty());
    assert_eq!(
        snapshot.legion_workflow_projection.redaction_hints,
        vec![RedactionHint::MetadataOnly]
    );
}

#[test]
fn legion_workflow_projection_roundtrips_without_ui_authority() {
    let mut snapshot = Shell::empty("legion").projection_snapshot();
    snapshot.legion_workflow_projection = test_legion_workflow_projection();

    let shell = Shell::new(snapshot.clone());
    let roundtrip = shell.projection_snapshot();

    assert_eq!(
        roundtrip.legion_workflow_projection,
        snapshot.legion_workflow_projection
    );
    assert_eq!(
        roundtrip.legion_workflow_projection.rows[0]
            .merge_readiness
            .state,
        legion_protocol::LegionWorkflowMergeReadinessState::WaitingForApproval
    );
    assert!(shell.command_dispatch_intents.is_empty());
}

#[test]
fn legion_workflow_command_center_fields_roundtrip_without_ui_authority() {
    let mut snapshot = Shell::empty("legion console").projection_snapshot();
    snapshot.legion_workflow_board_columns = vec![LegionWorkflowBoardColumnProjection {
        kind: LegionWorkflowBoardColumnKind::InProgress,
        title: "In Progress".to_string(),
        rows: vec![LegionWorkflowBoardRowProjection {
            session_id: LegionWorkflowSessionId("session:console".to_string()),
            state: LegionWorkflowState::Executing,
            state_label: "Executing".to_string(),
            summary_label: "session:console workers=1".to_string(),
        }],
    }];
    snapshot.legion_workflow_fleet_card_projections = vec![LegionWorkflowFleetCardProjection {
        proposal_id: ProposalId(99),
        title: "Console proposal".to_string(),
        owner_label: "owner:console".to_string(),
        model_label: "model:local".to_string(),
        status_label: "previewed".to_string(),
        progress_label: "projection-progress=1/1".to_string(),
        files_label: "manifest:console files=1".to_string(),
        risk_label: ProposalRiskLabel::Low,
        test_status_label: "passed=1 failed=0".to_string(),
        mini_diff_label: "metadata-only".to_string(),
        last_activity_label: "updated_at=7".to_string(),
    }];
    snapshot.legion_workflow_comm_rows =
        vec!["[2026-07-08T12:00:00Z] [PLAN] worker:console: metadata-only event".to_string()];
    snapshot.legion_workflow_budget_rows = vec![LegionWorkflowBudgetUsageRowProjection {
        session_id: LegionWorkflowSessionId("session:console".to_string()),
        worker_id: "worker:console".to_string(),
        budget_label: "loop".to_string(),
        model_turns_label: "model_turns=1/5".to_string(),
        tool_calls_label: "tool_calls=2/8".to_string(),
        retry_label: "retries=0/3".to_string(),
        output_bytes_label: "output_bytes=128/4096".to_string(),
        wall_clock_label: "wall_clock=10/1000ms".to_string(),
        status_label: "within-budget".to_string(),
        schema_version: 1,
    }];

    let mut shell = Shell::empty("legion console");
    shell.replace_projection_snapshot(snapshot.clone());
    let roundtrip = shell.projection_snapshot();

    assert_eq!(
        roundtrip.legion_workflow_board_columns,
        snapshot.legion_workflow_board_columns
    );
    assert_eq!(
        roundtrip.legion_workflow_fleet_card_projections,
        snapshot.legion_workflow_fleet_card_projections
    );
    assert_eq!(
        roundtrip.legion_workflow_comm_rows,
        snapshot.legion_workflow_comm_rows
    );
    assert_eq!(
        roundtrip.legion_workflow_budget_rows,
        snapshot.legion_workflow_budget_rows
    );
    assert!(shell.command_dispatch_intents.is_empty());
}

#[test]
fn legion_workflow_commands_emit_projection_only_intents() {
    let mut shell = Shell::empty("legion commands");

    let inspect = shell
        .handle_command(":legion-inspect session:legion:test")
        .expect("legion inspect parses")
        .expect("intent emitted");
    assert_eq!(
        inspect,
        CommandDispatchIntent::InspectLegionWorkflowSession {
            session_id: LegionWorkflowSessionId("session:legion:test".to_string())
        }
    );

    let verify = shell
        .handle_command(":legion-verify session:legion:test verification:unit")
        .expect("legion verification parses")
        .expect("intent emitted");
    assert_eq!(
        verify,
        CommandDispatchIntent::RequestLegionWorkflowVerification {
            session_id: LegionWorkflowSessionId("session:legion:test".to_string()),
            gate_id: LegionWorkflowVerificationGateId("verification:unit".to_string()),
        }
    );

    let readiness = shell
        .handle_command(":legion-readiness session:legion:test")
        .expect("legion readiness parses")
        .expect("intent emitted");
    assert_eq!(
        readiness,
        CommandDispatchIntent::RequestLegionWorkflowMergeReadiness {
            session_id: LegionWorkflowSessionId("session:legion:test".to_string())
        }
    );
    assert_eq!(shell.command_dispatch_intents.len(), 3);
}

#[test]
fn legion_workflow_malformed_command_does_not_emit_privileged_intent() {
    let mut shell = Shell::empty("legion malformed");
    let before = shell.projection_snapshot();

    assert_eq!(
        shell
            .handle_command(":legion-verify session-only")
            .expect("malformed command is ignored"),
        Some(CommandDispatchIntent::Noop)
    );
    assert_eq!(shell.projection_snapshot(), before);
    assert_eq!(
        shell.command_dispatch_intents,
        vec![CommandDispatchIntent::Noop]
    );
}

#[test]
fn ui_plugin_contributions_are_projection_only_command_intents() {
    let mut shell = Shell::empty("plugins");
    shell.plugin_contribution_projections = vec![PluginContributionProjection {
        plugin_id: PluginId(7),
        contributions: vec![legion_protocol::PluginContribution::Command(
            legion_protocol::PluginCommandDescriptor {
                command_id: "phase5.run".to_string(),
                title: "Phase 5 Run".to_string(),
                required_capability: CapabilityId("plugin.command".to_string()),
            },
        )],
        permission_review_rows: Vec::new(),
        status_label: "loaded".to_string(),
    }];

    let before = shell.projection_snapshot();
    let intent = shell
        .handle_command(":plugin 7 phase5.run metadata-only")
        .expect("plugin command should parse")
        .expect("intent should be emitted");

    assert_eq!(
        intent,
        CommandDispatchIntent::InvokePluginCommand {
            plugin_id: PluginId(7),
            command_id: "phase5.run".to_string(),
            metadata_label: "metadata-only".to_string(),
        }
    );
    assert_eq!(shell.projection_snapshot(), before);
    assert_eq!(shell.command_dispatch_intents.len(), 1);
}

#[test]
fn ui_collaboration_presence_is_projection_only_command_intent() {
    let mut shell = Shell::empty("collaboration");
    shell.collaboration_presence_projections = vec![CollaborationPresenceProjection {
        session_id: CollaborationSessionId(1001),
        participant_id: CollaborationParticipantId(2001),
        cursor: Some(test_coordinate(0, 0)),
        selections: Vec::new(),
        activity_label: Some("editing metadata-only range".to_string()),
        reconnecting: false,
        schema_version: 1,
    }];

    let before = shell.projection_snapshot();
    let intent = shell
        .handle_command(":collab-presence 1001 2001")
        .expect("collaboration command should parse")
        .expect("intent should be emitted");

    assert_eq!(
        intent,
        CommandDispatchIntent::PublishCollaborationPresence {
            session_id: CollaborationSessionId(1001),
            participant_id: CollaborationParticipantId(2001),
        }
    );
    assert_eq!(shell.projection_snapshot(), before);
    assert_eq!(shell.command_dispatch_intents.len(), 1);
}

#[test]
fn typescript_toolchain_intents_preserve_app_metadata_only() {
    let configure = CommandDispatchIntent::ConfigureTypeScriptToolchain {
        server_archive: "cache/typescript-language-server.tgz".to_string(),
        compiler_archive: "cache/typescript.tgz".to_string(),
        node_executable: "runtime/node".to_string(),
    };
    assert_eq!(
        configure,
        CommandDispatchIntent::ConfigureTypeScriptToolchain {
            server_archive: "cache/typescript-language-server.tgz".to_string(),
            compiler_archive: "cache/typescript.tgz".to_string(),
            node_executable: "runtime/node".to_string(),
        }
    );
    assert_eq!(
        CommandDispatchIntent::ClearTypeScriptToolchain,
        CommandDispatchIntent::ClearTypeScriptToolchain
    );
}

#[test]
fn explorer_projection_holds_nodes_and_selection() {
    let projection = ExplorerProjection {
        nodes: vec![ExplorerNodeProjection {
            file_id: FileId(10),
            canonical_path: CanonicalPath("C:/repo/src/main.rs".to_string()),
            name: "main.rs".to_string(),
            children: vec![],
            is_directory: false,
        }],
        selection: Some(ExplorerSelectionProjection {
            file_id: FileId(10),
        }),
    };

    assert_eq!(projection.nodes.len(), 1);
    assert_eq!(projection.nodes[0].name, "main.rs");
    assert_eq!(
        projection.selection.map(|sel| sel.file_id),
        Some(FileId(10))
    );
}

fn shell_with_small_buffer(text: &str) -> Shell {
    let mut shell = Shell::empty("t");
    shell.active_buffer_projection.buffer_id = Some(BufferId(2));
    shell.active_buffer_projection.small_buffer_preview = Some(text.to_string());
    shell.active_buffer_projection.viewport = None;
    shell
}

fn shell_with_viewport() -> Shell {
    let mut shell = Shell::empty("t");
    shell.active_buffer_projection.buffer_id = Some(BufferId(2));
    shell.active_buffer_projection.small_buffer_preview = None;
    shell.active_buffer_projection.viewport = Some(degraded_viewport_projection());
    shell
}

#[test]
fn sanitize_terminal_text_escapes_control_and_ansi_sequences() {
    // ESC-based ANSI clear-screen plus raw C0 controls are neutralized.
    let sanitized = sanitize_terminal_text("\x1b[2Jred\x07\rmalice");
    assert_eq!(sanitized, "\\x1b[2Jred\\x07\\x0dmalice");
    assert!(!sanitized.contains('\x1b'));
    // DEL (0x7f) and C1 controls (0x80-0x9f) are escaped too.
    assert_eq!(sanitize_terminal_text("\u{7f}\u{9b}"), "\\x7f\\x9b");
    // Newline and tab are preserved; ordinary (including multibyte) text passes through.
    assert_eq!(sanitize_terminal_text("a\n\tb"), "a\n\tb");
    assert_eq!(sanitize_terminal_text("héllo"), "héllo");
}

#[test]
fn parse_pos_rejects_out_of_bounds_offset() {
    let mut shell = shell_with_small_buffer("first");
    assert_eq!(
        shell.handle_command(":d 0,99").unwrap_err(),
        ShellCommandError::InvalidPosition
    );
}

#[test]
fn parse_pos_rejects_mid_codepoint_offset() {
    // 'é' occupies bytes 3..5, so offset 4 splits a UTF-8 character.
    let mut shell = shell_with_small_buffer("café");
    assert_eq!(
        shell.handle_command(":d 0,4").unwrap_err(),
        ShellCommandError::InvalidPosition
    );
}

#[test]
fn parse_pos_accepts_in_bounds_char_boundary_offsets() {
    let mut shell = shell_with_small_buffer("café");
    let intent = shell
        .handle_command(":d 0,5")
        .expect("in-bounds delete should parse")
        .expect("intent emitted");
    match intent {
        CommandDispatchIntent::Delete { range, .. } => {
            assert_eq!(range.start.byte_offset, Some(0));
            assert_eq!(range.end.byte_offset, Some(5));
        }
        other => panic!("expected delete intent, got {other:?}"),
    }
}

#[test]
fn parse_pos_viewport_returns_absolute_byte_offset() {
    let mut shell = shell_with_viewport();
    let intent = shell
        .handle_command(":d 0,5")
        .expect("viewport delete should parse")
        .expect("intent emitted");
    match intent {
        CommandDispatchIntent::Delete { range, .. } => {
            // First visible slice starts at absolute byte 1024 (not 0).
            assert_eq!(range.start.byte_offset, Some(1024));
            assert_eq!(range.end.byte_offset, Some(1029));
            assert_eq!(range.start.line, 10);
        }
        other => panic!("expected delete intent, got {other:?}"),
    }
}

#[test]
fn parse_pos_viewport_rejects_offset_outside_visible_slices() {
    let mut shell = shell_with_viewport();
    assert_eq!(
        shell.handle_command(":d 0,1000").unwrap_err(),
        ShellCommandError::InvalidPosition
    );
}

#[test]
fn parse_proposal_id_rejects_zero_sentinel() {
    assert_eq!(parse_proposal_id(Some("0")), None);
    assert_eq!(parse_proposal_id(Some("   ")), None);
    assert_eq!(parse_proposal_id(Some("42")), Some(ProposalId(42)));

    let mut shell = Shell::empty("t");
    let outcome = shell
        .handle_command(":proposal-approve 0")
        .expect("command should parse");
    // Zero is a reserved sentinel, so no ApproveProposal intent is emitted.
    assert_eq!(outcome, Some(CommandDispatchIntent::Noop));
    assert!(
        shell
            .command_dispatch_intents
            .iter()
            .all(|intent| !matches!(intent, CommandDispatchIntent::ApproveProposal { .. }))
    );
}

#[test]
fn toast_ids_distinguish_identical_status_messages() {
    let messages = vec![
        StatusMessageProjection {
            severity: StatusSeverity::Warning,
            message: "duplicate warning".to_string(),
        },
        StatusMessageProjection {
            severity: StatusSeverity::Warning,
            message: "duplicate warning".to_string(),
        },
    ];
    let stack = ToastStackProjection::from_status_messages(&messages, &[]);
    assert_eq!(stack.visible.len(), 2);
    assert_ne!(stack.visible[0].id, stack.visible[1].id);

    // Dismissing one identical toast must not collapse the other.
    let dismissed = stack.visible[0].id;
    let remaining = ToastStackProjection::from_status_messages(&messages, &[dismissed]);
    assert_eq!(remaining.visible.len(), 1);
    assert!(remaining.visible.iter().all(|toast| toast.id != dismissed));
}
