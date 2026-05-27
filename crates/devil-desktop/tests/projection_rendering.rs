use devil_desktop::view::DesktopProjectionViewModel;
use devil_protocol::{
    BufferId, BufferVersion, ByteRange, CanonicalPath, CapabilityId, CollaborationParticipantId,
    CollaborationPresenceProjection, CollaborationSessionId, ContextManifestEgressStatus,
    ContextManifestInclusionState, ContextManifestItem, ContextManifestItemCount,
    ContextManifestItemKind, FileFingerprint, FileId, PluginCommandDescriptor, PluginContribution,
    PluginContributionProjection, PluginId, PrincipalId, ProposalContextManifestSummary,
    ProposalDiffSummary, ProposalDiffSummaryKind, ProposalId, ProposalLedgerProjection,
    ProposalLedgerRow, ProposalLifecycleState, ProposalLifecycleStateDisplay, ProposalPayloadKind,
    ProposalPrivacyLabel, ProposalRiskLabel, ProposalRollbackAvailability, ProposalTargetCoverage,
    ProposalTargetCoverageKind, ProtocolTextRange, RedactionHint, SemanticPrivacyScope, SnapshotId,
    TextCoordinate, TimestampMillis, Utf16Position, Utf16Range, ViewportDimensions,
    ViewportLineSlice, ViewportLineTruncationState, ViewportProjection, ViewportProjectionMode,
    ViewportScroll, WorkspaceId,
};
use devil_ui::{
    ActiveBufferProjection, ExplorerNodeProjection, ExplorerProjection,
    ExplorerSelectionProjection, Shell, StatusMessageProjection, StatusSeverity,
};

fn coord(line: u32, character: u32, byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
    }
}

fn range(start: u64, end: u64) -> ProtocolTextRange {
    ProtocolTextRange {
        start: coord(0, start as u32, start),
        end: coord(0, end as u32, end),
    }
}

fn fingerprint(value: &str) -> FileFingerprint {
    FileFingerprint {
        algorithm: "test".to_string(),
        value: value.to_string(),
    }
}

fn populated_proposal_ledger() -> ProposalLedgerProjection {
    ProposalLedgerProjection {
        rows: vec![ProposalLedgerRow {
            proposal_id: ProposalId(7),
            workspace_id: Some(WorkspaceId(1)),
            title: "Save Cargo manifest".to_string(),
            payload_kind: ProposalPayloadKind::SaveFile,
            lifecycle: ProposalLifecycleStateDisplay {
                state: ProposalLifecycleState::Created,
                label: "created".to_string(),
                description: "Proposal created".to_string(),
            },
            principal: PrincipalId("desktop-test".to_string()),
            capability: CapabilityId("workspace.save".to_string()),
            created_at: TimestampMillis(1),
            updated_at: TimestampMillis(2),
            expires_at: None,
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            rollback: ProposalRollbackAvailability::BestEffort,
            target_coverage: ProposalTargetCoverage {
                coverage_kind: ProposalTargetCoverageKind::Complete,
                targets: Vec::new(),
                omitted_target_count: 0,
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            context_manifest: ProposalContextManifestSummary {
                manifest_id: "manifest:proposal:7".to_string(),
                category_count: 1,
                total_item_count: 1,
                omitted_item_count: 0,
                categories: Vec::new(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            diff_summary: ProposalDiffSummary {
                kind: ProposalDiffSummaryKind::MetadataOnly,
                target_count: 1,
                hunk_count: 1,
                inserted_line_count: 1,
                deleted_line_count: 0,
                omitted_hunk_count: 0,
                full_source_redacted: true,
                diff_hash: Some(fingerprint("diff")),
                chunks: Vec::new(),
                redaction_hints: vec![RedactionHint::MetadataOnly],
            },
            preview_warnings: Vec::new(),
            diagnostics: Vec::new(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        selected_proposal_id: Some(ProposalId(7)),
        omitted_row_count: 0,
        generated_at: TimestampMillis(3),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn context_item() -> ContextManifestItem {
    ContextManifestItem {
        item_id: "context:file:Cargo.toml".to_string(),
        kind: ContextManifestItemKind::File,
        inclusion: ContextManifestInclusionState::Included,
        workspace_id: Some(WorkspaceId(1)),
        file_id: Some(FileId(2)),
        buffer_id: Some(BufferId(3)),
        proposal_id: Some(ProposalId(7)),
        target_id: Some("target:manifest".to_string()),
        path: Some(CanonicalPath("Cargo.toml".to_string())),
        ranges: Vec::new(),
        counts: vec![ContextManifestItemCount {
            label: "files".to_string(),
            count: 1,
        }],
        hashes: Vec::new(),
        privacy_scope: Some(SemanticPrivacyScope::Workspace),
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        risk_label: ProposalRiskLabel::Low,
        egress: ContextManifestEgressStatus::LocalOnly,
        freshness: None,
        preconditions: None,
        labels: vec!["workspace manifest".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn populated_snapshot() -> devil_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Foundation Mode").projection_snapshot();
    snapshot.explorer_projection = ExplorerProjection {
        nodes: vec![ExplorerNodeProjection {
            file_id: FileId(2),
            canonical_path: CanonicalPath("Cargo.toml".to_string()),
            name: "Cargo.toml".to_string(),
            children: Vec::new(),
        }],
        selection: Some(ExplorerSelectionProjection { file_id: FileId(2) }),
    };
    snapshot.active_buffer_projection = ActiveBufferProjection {
        workspace_id: Some(WorkspaceId(1)),
        buffer_id: Some(BufferId(3)),
        file_id: Some(FileId(2)),
        file_path: Some(CanonicalPath("Cargo.toml".to_string())),
        viewport: None,
        degraded: false,
        small_buffer_preview: Some("[workspace]\nmembers = []".to_string()),
        dirty: true,
    };
    snapshot.status_messages = vec![StatusMessageProjection {
        severity: StatusSeverity::Info,
        message: "Desktop adapter ready".to_string(),
    }];
    snapshot.proposal_ledger_projection = populated_proposal_ledger();
    snapshot
        .context_manifest_projection
        .manifest
        .items
        .push(context_item());
    snapshot.assisted_ai_projection.provider_count = 1;
    snapshot.assisted_ai_projection.request_count = 1;
    snapshot.delegated_task_projection.plan_count = 1;
    snapshot.plugin_contribution_projections = vec![PluginContributionProjection {
        plugin_id: PluginId(4),
        contributions: vec![PluginContribution::Command(PluginCommandDescriptor {
            command_id: "phase2.command".to_string(),
            title: "Command".to_string(),
            required_capability: CapabilityId("plugin.command".to_string()),
        })],
        status_label: "loaded".to_string(),
    }];
    snapshot.collaboration_presence_projections = vec![CollaborationPresenceProjection {
        session_id: CollaborationSessionId(5),
        participant_id: CollaborationParticipantId(6),
        cursor: Some(coord(0, 1, 1)),
        selections: vec![range(0, 1)],
        activity_label: Some("editing".to_string()),
        reconnecting: false,
        schema_version: 1,
    }];
    snapshot
}

fn degraded_snapshot() -> devil_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Degraded").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        workspace_id: Some(WorkspaceId(1)),
        buffer_id: Some(BufferId(3)),
        file_id: Some(FileId(2)),
        file_path: Some(CanonicalPath("huge.rs".to_string())),
        viewport: Some(ViewportProjection {
            workspace_id: WorkspaceId(1),
            buffer_id: BufferId(3),
            file_id: Some(FileId(2)),
            snapshot_id: SnapshotId(4),
            buffer_version: BufferVersion(5),
            visible_range: range(0, 10),
            selections: Vec::new(),
            cursor: coord(0, 0, 0),
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 800,
                height_px: 600,
            },
            mode: ViewportProjectionMode::DegradedLargeFile,
            line_slices: vec![ViewportLineSlice {
                line_number: 0,
                visible_text: "visible degraded line".to_string(),
                byte_range: ByteRange::new(0, 21),
                utf16_range: Utf16Range {
                    start: Utf16Position {
                        line: 0,
                        character: 0,
                    },
                    end: Utf16Position {
                        line: 0,
                        character: 21,
                    },
                },
                chunk_hash: fingerprint("chunk"),
                truncation_state: ViewportLineTruncationState::Trailing,
            }],
            line_metrics: Vec::new(),
            decoration_spans: Vec::new(),
            fold_ranges: Vec::new(),
            semantic_token_overlays: Vec::new(),
            large_file_status: None,
            schema_version: 1,
        }),
        degraded: true,
        small_buffer_preview: None,
        dirty: false,
    };
    snapshot
}

#[test]
fn projection_rendering_populates_required_phase2_surfaces() {
    let model = DesktopProjectionViewModel::from_snapshot(&populated_snapshot());

    assert_eq!(model.layout_title, "Foundation Mode");
    assert!(
        model
            .explorer_rows
            .iter()
            .any(|row| row.contains("Cargo.toml"))
    );
    assert!(
        model
            .active_buffer_lines
            .iter()
            .any(|row| row.contains("[workspace]"))
    );
    assert!(
        model
            .status_rows
            .iter()
            .any(|row| row.contains("Desktop adapter ready"))
    );
    assert!(
        model
            .proposal_rows
            .iter()
            .any(|row| row.contains("Save Cargo manifest"))
    );
    assert!(
        model
            .trust_rows
            .iter()
            .any(|row| row.contains("context manifest"))
    );
    assert!(
        model
            .assistant_rows
            .iter()
            .any(|row| row.contains("assisted ai"))
    );
    assert!(model.plugin_rows.iter().any(|row| row.contains("plugin 4")));
    assert!(
        model
            .collaboration_rows
            .iter()
            .any(|row| row.contains("participant 6"))
    );
    assert!(model.empty_or_degraded_flags.contains(&"dirty".to_string()));
}

#[test]
fn projection_rendering_handles_empty_and_degraded_snapshots() {
    let empty_model =
        DesktopProjectionViewModel::from_snapshot(&Shell::empty("Empty").projection_snapshot());
    assert!(
        empty_model
            .explorer_rows
            .iter()
            .any(|row| row == "<empty explorer>")
    );
    assert!(
        empty_model
            .active_buffer_lines
            .iter()
            .any(|row| row == "<no active buffer>")
    );
    assert!(empty_model.proposal_rows.is_empty());
    assert!(empty_model.trust_rows.is_empty());
    assert!(empty_model.assistant_rows.is_empty());
    assert!(empty_model.plugin_rows.is_empty());
    assert!(empty_model.collaboration_rows.is_empty());

    let degraded_model = DesktopProjectionViewModel::from_snapshot(&degraded_snapshot());
    assert!(
        degraded_model
            .active_buffer_lines
            .iter()
            .any(|row| row.contains("visible degraded line"))
    );
    assert!(
        degraded_model
            .empty_or_degraded_flags
            .contains(&"degraded".to_string())
    );
}

#[test]
fn projection_rendering_tests_preserve_app_boundary() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/view.rs"))
        .expect("renderer source should be readable");

    assert!(!source.contains("devil_app"));
    assert!(!source.contains("AppComposition"));
}
