use std::collections::BTreeSet;

use legion_desktop::view::{
    DesktopCodeHighlightSpan, DesktopCodeLineViewModel, DesktopProjectionViewModel,
    DesktopProjectionViewState, drag_anchor_for_line_pointer, drag_selection_range,
    editor_coordinate_from_pointer, line_range_for_code_line, word_range_for_coordinate,
};
use legion_protocol::{
    ArtifactKind, ArtifactLedgerProjection, ArtifactLedgerRow, BufferId, BufferVersion, ByteRange,
    CanonicalPath, CapabilityId, CollaborationParticipantId, CollaborationPresenceProjection,
    CollaborationSessionId, CommandDescriptor, CommandRegistryProjection, CommandRiskLabel,
    ContextManifestEgressStatus, ContextManifestInclusionState, ContextManifestItem,
    ContextManifestItemCount, ContextManifestItemKind, FileFingerprint, FileId,
    PluginCommandDescriptor, PluginContribution, PluginContributionProjection, PluginId,
    PrincipalId, ProposalContextManifestSummary, ProposalDiffSummary, ProposalDiffSummaryKind,
    ProposalId, ProposalLedgerProjection, ProposalLedgerRow, ProposalLifecycleState,
    ProposalLifecycleStateDisplay, ProposalPayloadKind, ProposalPrivacyLabel, ProposalRiskLabel,
    ProposalRollbackAvailability, ProposalTargetCoverage, ProposalTargetCoverageKind,
    ProtocolTextRange, RedactionHint, SemanticPrivacyScope, SnapshotId, SystemGraphEdge,
    SystemGraphNode, SystemGraphProjection, TextCoordinate, TimestampMillis, Utf16Position,
    Utf16Range, VerificationRunProjection, VerificationRunRow, VerificationRunState,
    ViewportDimensions, ViewportLineSlice, ViewportLineTruncationState, ViewportProjection,
    ViewportProjectionMode, ViewportScroll, ViewportSemanticTokenKind,
    ViewportSemanticTokenOverlay, WorkspaceId,
};
use legion_ui::ui::{
    CloseDirtyPromptProjection, DailyEditingProjection, EditorTabProjection, EditorTabsProjection,
    EditorViewportStateProjection,
};
use legion_ui::{
    ActiveBufferProjection, AssistInlinePredictionProjection, AssistInlinePredictionRowProjection,
    AssistInlinePredictionStatusProjection, DockMode, ExplorerNodeProjection, ExplorerProjection,
    ExplorerSelectionProjection, PaletteMode, PaletteProjection, PaletteResult, PaletteResultKind,
    SearchScopeProjection, SettingsProjection, Shell, StatusMessageProjection, StatusSeverity,
    TOAST_VISIBLE_LIMIT, ThemePreferenceProjection, ToastVerbosityProjection,
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

fn populated_snapshot() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Foundation Mode").projection_snapshot();
    snapshot.product_mode = DockMode::Delegate;
    snapshot.explorer_projection = ExplorerProjection {
        nodes: vec![
            ExplorerNodeProjection {
                file_id: FileId(2),
                canonical_path: CanonicalPath("Cargo.toml".to_string()),
                name: "Cargo.toml".to_string(),
                children: vec![FileId(8)],
            },
            ExplorerNodeProjection {
                file_id: FileId(8),
                canonical_path: CanonicalPath("src/lib.rs".to_string()),
                name: "lib.rs".to_string(),
                children: Vec::new(),
            },
        ],
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
    snapshot.daily_editing_projection = DailyEditingProjection {
        tabs: EditorTabsProjection {
            tabs: vec![
                EditorTabProjection {
                    buffer_id: BufferId(3),
                    file_id: Some(FileId(2)),
                    file_path: Some(CanonicalPath("Cargo.toml".to_string())),
                    title: "Cargo.toml".to_string(),
                    active: true,
                    dirty: true,
                    pinned: false,
                    preview: false,
                },
                EditorTabProjection {
                    buffer_id: BufferId(9),
                    file_id: Some(FileId(8)),
                    file_path: Some(CanonicalPath("src/lib.rs".to_string())),
                    title: "lib.rs".to_string(),
                    active: false,
                    dirty: false,
                    pinned: true,
                    preview: false,
                },
            ],
            active_buffer_id: Some(BufferId(3)),
        },
        close_dirty_prompt: Some(CloseDirtyPromptProjection {
            buffer_id: BufferId(3),
            file_id: Some(FileId(2)),
            file_path: Some(CanonicalPath("Cargo.toml".to_string())),
            title: "Cargo.toml".to_string(),
            message: "Save changes before closing Cargo.toml?".to_string(),
        }),
        viewport_states: vec![EditorViewportStateProjection {
            buffer_id: BufferId(3),
            scroll: ViewportScroll {
                top_line: 2,
                left_column: 4,
            },
            cursor: Some(coord(1, 3, 12)),
            selections: vec![range(0, 1)],
        }],
        session_record: None,
    };
    snapshot.status_messages = vec![StatusMessageProjection {
        severity: StatusSeverity::Info,
        message: "Desktop adapter ready".to_string(),
    }];
    snapshot.command_registry_projection = CommandRegistryProjection {
        projection_id: "command-registry:test".to_string(),
        commands: vec![CommandDescriptor {
            command_id: "delegated.allocate_sandbox".to_string(),
            title: "Allocate Delegated Sandbox".to_string(),
            scope: "agents".to_string(),
            enabled: false,
            disabled_reason: Some("policy gate required".to_string()),
            shortcut: None,
            risk_label: CommandRiskLabel::Privileged,
            required_permission: Some(CapabilityId("delegated.runtime.allocate".to_string())),
            target: Some("isolated-worktree".to_string()),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        selected_command_id: None,
        omitted_command_count: 0,
        generated_at: TimestampMillis(4),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot.proposal_ledger_projection = populated_proposal_ledger();
    snapshot.artifact_ledger_projection = ArtifactLedgerProjection {
        projection_id: "artifact-ledger:test".to_string(),
        rows: vec![ArtifactLedgerRow {
            artifact_id: "artifact:approval:7".to_string(),
            kind: ArtifactKind::Approval,
            title: "Proposal approval".to_string(),
            state_label: "Created".to_string(),
            linked_proposal_id: Some(ProposalId(7)),
            linked_session_id: None,
            raw_payload_retained: false,
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        omitted_row_count: 0,
        generated_at: TimestampMillis(4),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot.verification_run_projection = VerificationRunProjection {
        projection_id: "verification-runs:test".to_string(),
        rows: vec![VerificationRunRow {
            run_id: "verification:test".to_string(),
            label: "cargo test".to_string(),
            state: VerificationRunState::Planned,
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
        generated_at: TimestampMillis(4),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
    snapshot.system_graph_projection = SystemGraphProjection {
        projection_id: "system-graph:test".to_string(),
        nodes: vec![SystemGraphNode {
            node_id: "system:workspace".to_string(),
            kind_label: "workspace".to_string(),
            display_label: "Active workspace".to_string(),
            target_count: 1,
            risk_label: ProposalRiskLabel::Low,
            privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        edges: vec![SystemGraphEdge {
            from_node_id: "system:workspace".to_string(),
            to_node_id: "system:proposal-ledger".to_string(),
            relation_label: "contains".to_string(),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        }],
        omitted_node_count: 0,
        omitted_edge_count: 0,
        generated_at: TimestampMillis(4),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    };
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

fn degraded_snapshot() -> legion_ui::ShellProjectionSnapshot {
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

fn highlighted_snapshot() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Highlighted").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        workspace_id: Some(WorkspaceId(1)),
        buffer_id: Some(BufferId(3)),
        file_id: Some(FileId(2)),
        file_path: Some(CanonicalPath("src/lib.rs".to_string())),
        viewport: Some(ViewportProjection {
            workspace_id: WorkspaceId(1),
            buffer_id: BufferId(3),
            file_id: Some(FileId(2)),
            snapshot_id: SnapshotId(4),
            buffer_version: BufferVersion(5),
            visible_range: range(0, 24),
            selections: Vec::new(),
            cursor: coord(0, 4, 4),
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 800,
                height_px: 600,
            },
            mode: ViewportProjectionMode::Normal,
            line_slices: vec![
                ViewportLineSlice {
                    line_number: 0,
                    visible_text: "pub fn answer() -> u32 {".to_string(),
                    byte_range: ByteRange::new(0, 24),
                    utf16_range: Utf16Range {
                        start: Utf16Position {
                            line: 0,
                            character: 0,
                        },
                        end: Utf16Position {
                            line: 0,
                            character: 24,
                        },
                    },
                    chunk_hash: fingerprint("chunk-0"),
                    truncation_state: ViewportLineTruncationState::None,
                },
                ViewportLineSlice {
                    line_number: 1,
                    visible_text: "    42".to_string(),
                    byte_range: ByteRange::new(25, 31),
                    utf16_range: Utf16Range {
                        start: Utf16Position {
                            line: 1,
                            character: 0,
                        },
                        end: Utf16Position {
                            line: 1,
                            character: 6,
                        },
                    },
                    chunk_hash: fingerprint("chunk-1"),
                    truncation_state: ViewportLineTruncationState::None,
                },
            ],
            line_metrics: Vec::new(),
            decoration_spans: Vec::new(),
            fold_ranges: Vec::new(),
            semantic_token_overlays: vec![
                ViewportSemanticTokenOverlay {
                    line_number: 0,
                    start_col: 0,
                    end_col: 3,
                    kind: ViewportSemanticTokenKind::Keyword,
                },
                ViewportSemanticTokenOverlay {
                    line_number: 1,
                    start_col: 4,
                    end_col: 6,
                    kind: ViewportSemanticTokenKind::Number,
                },
            ],
            large_file_status: None,
            schema_version: 2,
        }),
        degraded: false,
        small_buffer_preview: None,
        dirty: false,
    };
    snapshot
}

fn assist_inline_prediction_snapshot() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Assist").projection_snapshot();
    snapshot.product_mode = DockMode::Assist;
    snapshot.active_buffer_projection = ActiveBufferProjection {
        workspace_id: Some(WorkspaceId(1)),
        buffer_id: Some(BufferId(3)),
        file_id: Some(FileId(2)),
        file_path: Some(CanonicalPath("src/lib.rs".to_string())),
        viewport: None,
        degraded: false,
        small_buffer_preview: Some("let future = call();".to_string()),
        dirty: false,
    };
    snapshot.assist_inline_prediction_projection = AssistInlinePredictionProjection {
        active_prediction: Some(AssistInlinePredictionRowProjection {
            prediction_id: "assist:prediction:1".to_string(),
            workspace_id: Some(WorkspaceId(1)),
            buffer_id: Some(BufferId(3)),
            file_id: Some(FileId(2)),
            provider_label: "Local fixture".to_string(),
            status: AssistInlinePredictionStatusProjection::Ready,
            status_label: "ready".to_string(),
            latency_ms: Some(38),
            requested_at: TimestampMillis(100),
            completed_at: Some(TimestampMillis(138)),
            snapshot_id: Some(SnapshotId(5)),
            buffer_version: Some(BufferVersion(12)),
            file_fingerprint: Some(FileFingerprint {
                algorithm: "sha256".to_string(),
                value: "fingerprint-a".to_string(),
            }),
            stale: true,
            stale_reason_label: Some("buffer advanced after prediction".to_string()),
            ghost_text_label: ".await".to_string(),
            replacement_preview_label: Some("future.await".to_string()),
            apply_range: range(10, 10),
            apply_range_label: "0:10..0:10".to_string(),
            diagnostics: vec!["metadata-only display label".to_string()],
        }),
        rows: Vec::new(),
        request_in_flight: false,
        stale_prediction_count: 1,
        generated_at: TimestampMillis(150),
        schema_version: 1,
    };
    snapshot
}

#[test]
fn projection_rendering_populates_required_phase2_surfaces() {
    let model = DesktopProjectionViewModel::from_snapshot(&populated_snapshot());

    assert_eq!(model.layout_title, "Foundation Mode");
    assert!(
        model
            .top_bar_rows
            .iter()
            .any(|row| row.contains("command bar: Foundation Mode"))
    );
    assert!(
        model
            .top_bar_rows
            .iter()
            .any(|row| row.contains("registry=1"))
    );
    assert!(
        model
            .left_sidebar_rows
            .iter()
            .any(|row| row.contains("project sidebar"))
    );
    assert!(
        model
            .main_canvas_rows
            .iter()
            .any(|row| row.contains("code canvas"))
    );
    assert!(
        model
            .directive_panel_rows
            .iter()
            .any(|row| row.contains("directive dock") && row.contains("artifacts=1"))
    );
    assert!(
        model
            .bottom_console_rows
            .iter()
            .any(|row| row.contains("bottom console"))
    );
    assert_eq!(model.status_bar.product_mode, "Delegate");
    assert_eq!(model.status_bar.flags, vec!["dirty".to_string()]);
    assert_eq!(model.status_bar.path.as_deref(), Some("Cargo.toml"));
    assert_eq!(model.status_bar.encoding.as_deref(), Some("UTF-8"));
    assert_eq!(model.status_bar.line_ending.as_deref(), Some("LF"));
    assert_eq!(model.status_bar.language.as_deref(), Some("toml"));
    assert_eq!(model.status_bar.connection, None);
    assert!(
        model
            .tab_rows
            .iter()
            .any(|row| row.contains("Cargo.toml +"))
    );
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
            .editor_status_rows
            .iter()
            .any(|row| row.contains("dirty small-buffer"))
    );
    assert!(
        model
            .viewport_metadata_rows
            .iter()
            .any(|row| row.contains("scroll=2:4"))
    );
    assert!(
        model
            .close_prompt_rows
            .iter()
            .any(|row| row.contains("close_dirty"))
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
fn projection_rendering_surfaces_assist_inline_prediction_rows() {
    let model = DesktopProjectionViewModel::from_snapshot(&assist_inline_prediction_snapshot());

    assert!(
        model
            .product_mode_rows
            .iter()
            .any(|row| { row.contains("active=Assist app-owned projection") })
    );
    assert!(model.main_canvas_rows.iter().any(|row| {
        row.contains("ghost prediction")
            && row.contains("provider=Local fixture")
            && row.contains("status=Ready")
            && row.contains("range=0:10..0:10")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("inline prediction assist:prediction:1")
            && row.contains("provider=Local fixture")
            && row.contains("latency=38ms")
            && row.contains("stale=true")
            && row.contains("fingerprint=sha256:fingerprint-a")
            && row.contains("ghost=.await")
            && row.contains("replacement=future.await")
    }));
    assert!(model.bottom_tab_rows.iter().any(|row| {
        row.contains("mode=Assist")
            && row.contains("id=sugg")
            && row.contains("label=AI Suggestions")
            && row.contains("count=1")
    }));
}

#[test]
fn projection_rendering_models_read_only_product_mode_shell() {
    let populated = DesktopProjectionViewModel::from_snapshot(&populated_snapshot());
    assert!(
        populated
            .product_mode_rows
            .iter()
            .any(|row| row.contains("active=Delegates app-owned projection"))
    );
    assert!(populated.product_mode_rows.iter().any(|row| {
        row.contains("approval-gated") && row.contains("direct workspace apply unsupported")
    }));
    assert!(
        populated
            .product_mode_rows
            .iter()
            .any(|row| row.contains("no provider, terminal, or apply authority"))
    );

    let empty =
        DesktopProjectionViewModel::from_snapshot(&Shell::empty("Manual").projection_snapshot());
    assert!(
        empty
            .product_mode_rows
            .iter()
            .any(|row| row.contains("active=Manual app-owned projection"))
    );
    assert!(
        empty
            .product_mode_rows
            .iter()
            .any(|row| row.contains("Manual Mode has no AI dispatch path"))
    );
    assert!(empty.manual_control_rows.iter().any(|row| {
        row.contains("AI Disabled")
            && row.contains("Local Tools Only")
            && row.contains("No Model Calls")
    }));
    assert!(empty.manual_control_rows.iter().any(|row| {
        row.contains("save_all proposal-mediated") && row.contains("no direct apply")
    }));
}

#[test]
fn projection_rendering_models_wireframe_chrome_contract() {
    let manual =
        DesktopProjectionViewModel::from_snapshot(&Shell::empty("Manual").projection_snapshot());
    assert!(manual.autonomy_scale_rows.iter().any(|row| {
        row.contains("label=Manual") && row.contains("active=true") && row.contains("key=M")
    }));
    assert!(manual.mode_confirmation_rows.iter().any(|row| {
        row.contains("target=Delegate")
            && row.contains("required=true")
            && row.contains("require_approval=true")
            && row.contains("allow_tests=true")
            && row.contains("allow_terminal=false")
            && row.contains("allow_dependency_install=false")
            && row.contains("protected=[.env,secrets/,*.pem]")
    }));
    assert!(manual.mode_confirmation_rows.iter().any(|row| {
        row.contains("target=Automate")
            && row.contains("required=true")
            && row.contains("allow_dependency_install=true")
    }));
    assert!(!manual.command_palette_overlay.open);
    assert!(manual.bottom_tab_rows.iter().any(|row| {
        row.contains("mode=Manual")
            && row.contains("id=term")
            && row.contains("label=Terminal")
            && row.contains("active=true")
    }));
    assert!(manual.bottom_tab_rows.iter().any(|row| {
        row.contains("mode=Manual") && row.contains("id=test") && row.contains("label=Tests")
    }));

    let mut assisted = Shell::empty("Assist").projection_snapshot();
    assisted.product_mode = DockMode::Assist;
    assisted.assisted_ai_projection.request_count = 1;
    let assisted_model = DesktopProjectionViewModel::from_snapshot(&assisted);
    assert!(assisted_model.autonomy_scale_rows.iter().any(|row| {
        row.contains("label=Assist") && row.contains("active=true") && row.contains("key=A")
    }));
    assert!(assisted_model.bottom_tab_rows.iter().any(|row| {
        row.contains("mode=Assist")
            && row.contains("id=sugg")
            && row.contains("label=AI Suggestions")
            && row.contains("count=1")
    }));

    let delegated = DesktopProjectionViewModel::from_snapshot(&populated_snapshot());
    assert!(delegated.autonomy_scale_rows.iter().any(|row| {
        row.contains("label=Delegate")
            && row.contains("active=true")
            && row.contains("confirm=required")
    }));
    assert!(!delegated.command_palette_overlay.open);
    assert!(delegated.bottom_tab_rows.iter().any(|row| {
        row.contains("mode=Delegates")
            && row.contains("id=test")
            && row.contains("label=Test Runner")
            && row.contains("active=true")
    }));
}

#[test]
fn projection_rendering_models_structured_command_palette_overlay() {
    let mut snapshot = Shell::empty("Palette").projection_snapshot();
    snapshot.palette_projection = PaletteProjection {
        open: true,
        mode: PaletteMode::File,
        query: "car".to_string(),
        scope: SearchScopeProjection::ActiveFile,
        selected_index: 0,
        results: vec![
            PaletteResult {
                id: "file:Cargo.toml".to_string(),
                kind: PaletteResultKind::File,
                title: "Cargo.toml".to_string(),
                detail: Some("workspace file".to_string()),
                shortcut_label: Some("Enter".to_string()),
                match_indices: vec![0, 1, 2],
                disabled_reason: None,
            },
            PaletteResult {
                id: "command:save-all".to_string(),
                kind: PaletteResultKind::Command,
                title: "Save All".to_string(),
                detail: Some("Save every open tab".to_string()),
                shortcut_label: Some("Ctrl+Shift+S".to_string()),
                match_indices: Vec::new(),
                disabled_reason: Some("No dirty tabs".to_string()),
            },
        ],
    };

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(model.command_palette_overlay.open);
    assert_eq!(model.command_palette_overlay.mode_label, "Files");
    assert_eq!(model.command_palette_overlay.query, "car");
    assert_eq!(model.command_palette_overlay.result_rows.len(), 2);
    assert!(model.command_palette_overlay.result_rows[0].selected);
    assert_eq!(
        model.command_palette_overlay.result_rows[0].shortcut_label,
        Some("Enter".to_string())
    );
    assert_eq!(
        model.command_palette_overlay.result_rows[1].disabled_reason,
        Some("No dirty tabs".to_string())
    );
}

#[test]
fn projection_rendering_keeps_selected_palette_result_visible_in_overlay_window() {
    let mut snapshot = Shell::empty("Palette").projection_snapshot();
    snapshot.palette_projection = PaletteProjection {
        open: true,
        mode: PaletteMode::File,
        query: String::new(),
        scope: SearchScopeProjection::Workspace,
        selected_index: 12,
        results: (0..15)
            .map(|index| PaletteResult {
                id: format!("file:item-{index}"),
                kind: PaletteResultKind::File,
                title: format!("item-{index}.rs"),
                detail: Some("workspace file".to_string()),
                shortcut_label: Some("Enter".to_string()),
                match_indices: Vec::new(),
                disabled_reason: None,
            })
            .collect(),
    };

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    let rows = &model.command_palette_overlay.result_rows;

    assert_eq!(rows.len(), 10);
    assert_eq!(rows.first().map(|row| row.id.as_str()), Some("file:item-3"));
    assert_eq!(rows.last().map(|row| row.id.as_str()), Some("file:item-12"));
    assert!(rows.last().is_some_and(|row| row.selected));
    assert!(
        model
            .command_palette_rows
            .iter()
            .any(|row| row.contains("selected=12") && row.contains("results=15"))
    );
}

#[test]
fn projection_rendering_models_warning_and_error_statuses_as_toasts() {
    let mut snapshot = Shell::empty("Toasts").projection_snapshot();
    snapshot.status_messages = vec![
        StatusMessageProjection {
            severity: StatusSeverity::Info,
            message: "Desktop adapter ready".to_string(),
        },
        StatusMessageProjection {
            severity: StatusSeverity::Warning,
            message: "Session restore skipped: workspace mismatch".to_string(),
        },
        StatusMessageProjection {
            severity: StatusSeverity::Error,
            message: "Save failed: stale buffer".to_string(),
        },
    ];

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert_eq!(model.toast_stack.visible.len(), 2);
    assert_eq!(model.toast_stack.visible[0].severity, StatusSeverity::Error);
    assert_eq!(model.toast_stack.visible[0].title, "Save failed");
    assert_eq!(
        model.toast_stack.visible[0].body.as_deref(),
        Some("stale buffer")
    );
    assert!(model.toast_stack.visible[0].sticky);
    assert_eq!(
        model.toast_stack.visible[1].title,
        "Session restore skipped"
    );
    assert_eq!(model.toast_stack.overflow_count, 0);

    snapshot.settings_projection.toast_verbosity = ToastVerbosityProjection::All;
    let all_model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    assert_eq!(all_model.toast_stack.visible.len(), 3);
    assert_eq!(
        all_model.toast_stack.visible[2].severity,
        StatusSeverity::Info
    );
}

#[test]
fn projection_rendering_bounds_and_dismisses_toasts() {
    let mut snapshot = Shell::empty("Toasts").projection_snapshot();
    snapshot.status_messages = (0..(TOAST_VISIBLE_LIMIT + 2))
        .map(|index| StatusMessageProjection {
            severity: StatusSeverity::Warning,
            message: format!("Warning {index}: detail"),
        })
        .collect();
    let initial = DesktopProjectionViewModel::from_snapshot(&snapshot);
    let dismissed_id = initial.toast_stack.visible[0].id;

    assert_eq!(initial.toast_stack.visible.len(), TOAST_VISIBLE_LIMIT);
    assert_eq!(initial.toast_stack.overflow_count, 2);

    let mut dismissed = BTreeSet::new();
    dismissed.insert(dismissed_id);
    let model = DesktopProjectionViewModel::from_snapshot_with_state(
        &snapshot,
        &DesktopProjectionViewState {
            dismissed_toast_ids: dismissed,
            ..DesktopProjectionViewState::default()
        },
    );

    assert_eq!(model.toast_stack.visible.len(), TOAST_VISIBLE_LIMIT);
    assert_eq!(model.toast_stack.overflow_count, 1);
    assert!(
        model
            .toast_stack
            .visible
            .iter()
            .all(|toast| toast.id != dismissed_id)
    );
}

#[test]
fn projection_rendering_uses_mode_filtered_dock_registry() {
    let empty =
        DesktopProjectionViewModel::from_snapshot(&Shell::empty("Manual").projection_snapshot());
    assert!(
        empty
            .dock_rows
            .iter()
            .any(|row| row.contains("mode=Manual"))
    );
    assert!(
        empty
            .dock_panel_rows
            .iter()
            .all(|row| row.contains("requires_ai=false")),
        "manual dock rows must not include AI-backed panels: {:?}",
        empty.dock_panel_rows
    );
    assert!(
        empty
            .dock_panel_rows
            .iter()
            .any(|row| row.contains("id=project_explorer"))
    );
    assert!(
        empty
            .dock_panel_rows
            .iter()
            .any(|row| row.contains("id=settings") && row.contains("requires_ai=false"))
    );

    let delegated = DesktopProjectionViewModel::from_snapshot(&populated_snapshot());
    assert!(
        delegated
            .dock_rows
            .iter()
            .any(|row| row.contains("mode=Delegate"))
    );
    assert!(
        delegated
            .dock_panel_rows
            .iter()
            .any(|row| row.contains("id=delegation") && row.contains("requires_ai=true"))
    );

    let mut assisted = Shell::empty("Assist").projection_snapshot();
    assisted.product_mode = DockMode::Assist;
    assisted.assisted_ai_projection.request_count = 1;
    let assisted_model = DesktopProjectionViewModel::from_snapshot(&assisted);
    assert!(
        assisted_model
            .product_mode_rows
            .iter()
            .any(|row| row.contains("active=Assist app-owned projection"))
    );
    assert!(
        assisted_model
            .dock_panel_rows
            .iter()
            .any(|row| row.contains("id=assistant") && row.contains("requires_ai=true"))
    );
}

#[test]
fn projection_rendering_projects_workbench_settings_model() {
    let mut snapshot = Shell::empty("Settings").projection_snapshot();
    snapshot.settings_projection = SettingsProjection {
        theme_preference: ThemePreferenceProjection::System,
        zoom_percent: 220,
        editor_font_size_pt: 8,
        toast_verbosity: ToastVerbosityProjection::All,
        editor: legion_ui::EditorSettingsProjection {
            line_numbers_visible: false,
            current_line_highlight: false,
        },
        schema_version: 0,
    };

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert_eq!(
        model.settings.theme_preference,
        ThemePreferenceProjection::System
    );
    assert_eq!(model.settings.theme_label, "System");
    assert_eq!(
        model.settings.zoom_percent,
        SettingsProjection::MAX_ZOOM_PERCENT
    );
    assert_eq!(
        model.settings.editor_font_size_pt,
        SettingsProjection::MIN_EDITOR_FONT_SIZE_PT
    );
    assert_eq!(
        model.settings.toast_verbosity,
        ToastVerbosityProjection::All
    );
    assert_eq!(model.settings.toast_verbosity_label, "All statuses");
    assert!(!model.settings.line_numbers_visible);
    assert!(!model.settings.current_line_highlight);
    assert_eq!(model.settings.schema_version, 1);
}

#[test]
fn projection_rendering_keeps_advanced_surfaces_metadata_and_projection_derived() {
    let model = DesktopProjectionViewModel::from_snapshot(&populated_snapshot());

    assert!(
        model
            .directive_panel_rows
            .iter()
            .any(|row| row.contains("proposal-mediated"))
    );
    assert!(
        model
            .bottom_console_rows
            .iter()
            .any(|row| row.contains("verification_runs=1") && row.contains("graph_nodes=1"))
    );
    assert!(
        model
            .assistant_rows
            .iter()
            .any(|row| row.contains("autonomous_apply=unsupported"))
    );
    assert!(model.plugin_rows.iter().any(|row| {
        row.contains("sandbox=metadata-only") || row.contains("dispatch-intent-only")
    }));
    assert!(
        model
            .collaboration_rows
            .iter()
            .any(|row| row.contains("redaction=metadata-only"))
    );
    assert!(
        model
            .directive_panel_rows
            .iter()
            .any(|row| row.contains("remote=0"))
    );
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
    assert!(empty_model.tab_rows.contains(&"<no open tabs>".to_string()));
    assert!(
        empty_model
            .editor_status_rows
            .contains(&"editor: no active buffer".to_string())
    );
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
    assert!(
        degraded_model
            .status_bar
            .flags
            .contains(&"degraded".to_string())
    );
    assert_eq!(degraded_model.status_bar.path.as_deref(), Some("huge.rs"));
    assert_eq!(
        degraded_model.status_bar.cursor,
        Some(legion_desktop::view::DesktopStatusCursor { line: 1, column: 1 })
    );
    assert!(
        degraded_model
            .editor_status_rows
            .iter()
            .any(|row| row.contains("DegradedLargeFile"))
    );
}

#[test]
fn projection_rendering_preserves_semantic_token_spans_for_code_canvas() {
    let model = DesktopProjectionViewModel::from_snapshot(&highlighted_snapshot());

    assert_eq!(model.active_buffer_code_lines.len(), 2);
    assert_eq!(model.active_buffer_code_lines[0].number, 1);
    assert_eq!(
        model.active_buffer_code_lines[0].text,
        "pub fn answer() -> u32 {"
    );
    assert!(
        model.active_buffer_code_lines[0]
            .highlights
            .iter()
            .any(|span| {
                span.start_col == 0
                    && span.end_col == 3
                    && span.kind == ViewportSemanticTokenKind::Keyword
            })
    );
    assert!(
        model.active_buffer_code_lines[1]
            .highlights
            .iter()
            .any(|span| {
                span.start_col == 4
                    && span.end_col == 6
                    && span.kind == ViewportSemanticTokenKind::Number
            })
    );
    assert!(
        model
            .active_buffer_lines
            .iter()
            .any(|row| row.contains("pub fn answer"))
    );
}

#[test]
fn projection_rendering_maps_editor_pointer_to_text_coordinate() {
    let lines = vec![
        DesktopCodeLineViewModel {
            number: 4,
            text: "alpha".to_string(),
            highlights: vec![DesktopCodeHighlightSpan {
                start_col: 0,
                end_col: 5,
                kind: ViewportSemanticTokenKind::Ident,
            }],
        },
        DesktopCodeLineViewModel {
            number: 5,
            text: "beta_value".to_string(),
            highlights: Vec::new(),
        },
    ];

    let coordinate = editor_coordinate_from_pointer(
        egui::pos2(34.0, 42.0),
        egui::pos2(10.0, 20.0),
        18.0,
        8.0,
        &lines,
    )
    .expect("pointer should map to second row");

    assert_eq!(coordinate.line, 4);
    assert_eq!(coordinate.character, 3);
    assert_eq!(coordinate.byte_offset, None);
    assert_eq!(coordinate.utf16_offset, None);

    let clamped = editor_coordinate_from_pointer(
        egui::pos2(400.0, 20.0),
        egui::pos2(10.0, 20.0),
        18.0,
        8.0,
        &lines,
    )
    .expect("pointer should clamp to first row end");
    assert_eq!(clamped.line, 3);
    assert_eq!(clamped.character, 5);
}

#[test]
fn projection_rendering_computes_word_and_line_selection_ranges() {
    let line = DesktopCodeLineViewModel {
        number: 8,
        text: "let beta_value = 42;".to_string(),
        highlights: Vec::new(),
    };
    let word = word_range_for_coordinate(&line, coord(7, 6, 0)).expect("word range");
    assert_eq!(word.start.line, 7);
    assert_eq!(word.start.character, 4);
    assert_eq!(word.end.line, 7);
    assert_eq!(word.end.character, 14);

    let full_line = line_range_for_code_line(&line);
    assert_eq!(full_line.start.line, 7);
    assert_eq!(full_line.start.character, 0);
    assert_eq!(full_line.end.line, 7);
    assert_eq!(full_line.end.character, 20);
}

#[test]
fn projection_rendering_anchors_drag_selection_at_gesture_start() {
    let line = DesktopCodeLineViewModel {
        number: 8,
        text: "let beta_value = 42;".to_string(),
        highlights: Vec::new(),
    };
    let old_cursor = coord(20, 0, 0);
    let end = coord(7, 14, 14);
    let anchor = drag_anchor_for_line_pointer(&line, 74.0, egui::vec2(32.0, 0.0), 10.0, 8.0);
    let range = drag_selection_range(Some(anchor), old_cursor, end);

    assert_eq!(range.start.line, 7);
    assert_eq!(range.start.character, 4);
    assert_eq!(range.end, end);

    let fallback = drag_selection_range(None, old_cursor, end);
    assert_eq!(fallback.start, old_cursor);
    assert_eq!(fallback.end, end);
}

#[test]
fn projection_rendering_marks_expanded_and_collapsed_explorer_rows() {
    let snapshot = populated_snapshot();
    let collapsed = DesktopProjectionViewModel::from_snapshot(&snapshot);
    assert!(
        collapsed
            .explorer_state_rows
            .iter()
            .any(|row| row.contains("> Cargo.toml"))
    );
    assert!(
        !collapsed
            .explorer_state_rows
            .iter()
            .any(|row| row.contains("lib.rs"))
    );

    let mut expanded = BTreeSet::new();
    expanded.insert("Cargo.toml".to_string());
    let model = DesktopProjectionViewModel::from_snapshot_with_state(
        &snapshot,
        &DesktopProjectionViewState {
            expanded_explorer_paths: expanded,
            selected_explorer_file: Some(FileId(8)),
            ..DesktopProjectionViewState::default()
        },
    );
    assert!(
        model
            .explorer_state_rows
            .iter()
            .any(|row| row.contains("v Cargo.toml"))
    );
    assert!(
        model
            .explorer_state_rows
            .iter()
            .any(|row| row.contains("* -   lib.rs"))
    );
}

#[test]
fn projection_rendering_tests_preserve_app_boundary() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/view.rs"))
        .expect("renderer source should be readable");

    assert!(!source.contains("legion_app"));
    assert!(!source.contains("AppComposition"));
}
