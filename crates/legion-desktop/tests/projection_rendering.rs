use std::collections::BTreeSet;

use legion_desktop::bridge::DesktopAction;
use legion_desktop::view::ShellGeometry;
use legion_desktop::view::{
    BottomPanelTab, DesktopCodeHighlightSpan, DesktopCodeLineViewModel, DesktopProjectionViewModel,
    DesktopProjectionViewState, ProjectionView, ProjectionViewOutput, drag_anchor_for_line_pointer,
    drag_selection_range, editor_coordinate_from_pointer, line_range_for_code_line,
    word_range_for_coordinate,
};
use legion_protocol::LanguageCodeLensProjection;
use legion_protocol::{
    ArtifactKind, ArtifactLedgerProjection, ArtifactLedgerRow, BufferId, BufferVersion, ByteRange,
    CanonicalPath, CapabilityId, CollaborationParticipantId, CollaborationPresenceProjection,
    CollaborationSessionId, CommandDescriptor, CommandRegistryProjection, CommandRiskLabel,
    ContextManifestEgressStatus, ContextManifestInclusionState, ContextManifestItem,
    ContextManifestItemCount, ContextManifestItemKind, FileFingerprint, FileId,
    LanguageStickyScopeProjection, LargeFileStatus, LineWrappingPolicy, PluginCommandDescriptor,
    PluginContribution, PluginContributionProjection, PluginId, PrincipalId,
    ProposalContextManifestSummary, ProposalDiffSummary, ProposalDiffSummaryKind, ProposalId,
    ProposalLedgerProjection, ProposalLedgerRow, ProposalLifecycleState,
    ProposalLifecycleStateDisplay, ProposalPayloadKind, ProposalPrivacyLabel, ProposalRiskLabel,
    ProposalRollbackAvailability, ProposalTargetCoverage, ProposalTargetCoverageKind,
    ProtocolTextRange, RedactionHint, SemanticPrivacyScope, SnapshotId, SystemGraphEdge,
    SystemGraphNode, SystemGraphProjection, TextCoordinate, TimestampMillis, Utf16Position,
    Utf16Range, VerificationRunProjection, VerificationRunRow, VerificationRunState,
    ViewportDimensions, ViewportFoldRange, ViewportLineSlice, ViewportLineTruncationState,
    ViewportProjection, ViewportProjectionMode, ViewportScroll, ViewportSemanticTokenKind,
    ViewportSemanticTokenOverlay, WorkspaceId,
};
use legion_ui::ui::{
    CloseDirtyPromptProjection, DailyEditingProjection, EditorTabProjection, EditorTabsProjection,
    EditorViewportStateProjection,
};
use legion_ui::{
    ActiveBufferProjection, ActiveBufferProjectionState, AssistInlinePredictionProjection,
    AssistInlinePredictionRowProjection, AssistInlinePredictionStatusProjection, DockMode,
    ExplorerNodeProjection, ExplorerProjection, ExplorerSelectionProjection, PaletteMode,
    PaletteProjection, PaletteResult, PaletteResultKind, SearchScopeProjection, SettingsProjection,
    Shell, StatusMessageProjection, StatusSeverity, TOAST_VISIBLE_LIMIT, ThemePreferenceProjection,
    ToastVerbosityProjection,
};

mod common;

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
        state: ActiveBufferProjectionState::Full,
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
    snapshot.language_tooling_projection.code_lenses = vec![LanguageCodeLensProjection {
        lens_id: "lens:test-run".to_string(),
        title: "Run test".to_string(),
        command_label: "rust-analyzer.runSingle".to_string(),
        kind_label: "lsp.codelens.runnable".to_string(),
        range: Some(range(0, 3)),
        data_label: Some("kind=runnable".to_string()),
        source_label: "rust-analyzer".to_string(),
        schema_version: 1,
    }];
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
        plugin_id: PluginId(7),
        contributions: vec![PluginContribution::Command(PluginCommandDescriptor {
            command_id: "phase5.run".to_string(),
            title: "Phase 5 Run".to_string(),
            required_capability: CapabilityId("plugin.command".to_string()),
        })],
        permission_review_rows: vec![
            "permission review 1: capability=plugin.command reason=command phase5.run".to_string(),
        ],
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
        state: ActiveBufferProjectionState::Full,
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
            cursors: vec![coord(0, 0, 0)],
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 800,
                height_px: 600,
            },
            line_wrapping_policy: LineWrappingPolicy::Off,
            wrap_column: None,
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
            large_file_status: Some(LargeFileStatus {
                threshold_bytes: 16,
                byte_len: 64,
                disabled_overlay_reasons: vec![
                    "semantic overlays disabled".to_string(),
                    "C:\\Windows\\Fonts\\malgun.ttf\nHIDDEN_NEEDLE_AFTER_VIEWPORT".to_string(),
                    "/System/Library/Fonts/PingFang.ttc\u{0000}".to_string(),
                    "/usr/share/fonts/noto/NotoSansCJK.ttc".to_string(),
                ],
                bounded_search_enabled: true,
                message: "degraded large file".to_string(),
            }),
            schema_version: 1,
        }),
        degraded: true,
        small_buffer_preview: None,
        dirty: false,
    };
    snapshot
}

fn streaming_snapshot() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Streaming").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        state: ActiveBufferProjectionState::Full,
        workspace_id: Some(WorkspaceId(1)),
        buffer_id: Some(BufferId(7)),
        file_id: Some(FileId(9)),
        file_path: Some(CanonicalPath("streamed.rs".to_string())),
        viewport: Some(ViewportProjection {
            workspace_id: WorkspaceId(1),
            buffer_id: BufferId(7),
            file_id: Some(FileId(9)),
            snapshot_id: SnapshotId(12),
            buffer_version: BufferVersion(13),
            visible_range: range(0, 8),
            selections: Vec::new(),
            cursor: coord(0, 0, 0),
            cursors: vec![coord(0, 0, 0)],
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 800,
                height_px: 600,
            },
            line_wrapping_policy: LineWrappingPolicy::Off,
            wrap_column: None,
            mode: ViewportProjectionMode::Normal,
            line_slices: vec![ViewportLineSlice {
                line_number: 0,
                visible_text: "visible streaming line".to_string(),
                byte_range: ByteRange::new(0, 23),
                utf16_range: Utf16Range {
                    start: Utf16Position {
                        line: 0,
                        character: 0,
                    },
                    end: Utf16Position {
                        line: 0,
                        character: 23,
                    },
                },
                chunk_hash: fingerprint("chunk-streaming"),
                truncation_state: ViewportLineTruncationState::None,
            }],
            line_metrics: Vec::new(),
            decoration_spans: Vec::new(),
            fold_ranges: Vec::new(),
            semantic_token_overlays: Vec::new(),
            large_file_status: None,
            schema_version: 2,
        }),
        degraded: false,
        small_buffer_preview: None,
        dirty: false,
    };
    snapshot
}

fn highlighted_snapshot() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Highlighted").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        state: ActiveBufferProjectionState::Full,
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
            cursors: vec![coord(0, 4, 4)],
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 800,
                height_px: 600,
            },
            line_wrapping_policy: LineWrappingPolicy::Off,
            wrap_column: None,
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
        state: ActiveBufferProjectionState::Full,
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
        after_edit_prediction_attempts: 0,
        after_edit_prediction_accepts: 0,
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
            .any(|row| row.contains("top bar identity: Legion workspace=Foundation Mode"))
    );
    assert!(
        model
            .top_bar_rows
            .iter()
            .any(|row| row.contains("active=Delegate"))
    );
    assert!(
        model
            .left_sidebar_rows
            .iter()
            .any(|row| row.contains("explorer chrome"))
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
    assert!(model.test_rows.iter().any(|row| {
        row.contains("test explorer: verification_runs=1")
            && row.contains("runnable_lenses=1")
            && row.contains("projection=verification-runs:test")
    }));
    assert!(
        model
            .test_rows
            .iter()
            .any(|row| { row.contains("run verification:test") && row.contains("state=Planned") })
    );
    assert!(model.test_rows.iter().any(|row| {
        row.contains("runnable lens")
            && row.contains("Run test")
            && row.contains("rust-analyzer.runSingle")
    }));
    assert!(model.plugin_rows.iter().any(
        |row| row.contains("permission review 1") && row.contains("capability=plugin.command")
    ));
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
            && row.contains("id=agent-log")
            && row.contains("label=AGENT LOG")
    }));
}

#[test]
fn projection_rendering_models_read_only_product_mode_shell() {
    let populated = DesktopProjectionViewModel::from_snapshot(&populated_snapshot());
    assert!(
        populated
            .product_mode_rows
            .iter()
            .any(|row| row.contains("active=Delegate app-owned projection"))
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
        row.contains("target=Legion Workflows")
            && row.contains("required=true")
            && row.contains("allow_dependency_install=true")
    }));
    assert!(!manual.command_palette_overlay.open);
    assert!(manual.bottom_tab_rows.iter().any(|row| {
        row.contains("mode=Manual")
            && row.contains("id=term")
            && row.contains("label=TERMINAL")
            && row.contains("active=true")
    }));
    assert!(manual.bottom_tab_rows.iter().any(|row| {
        row.contains("mode=Manual") && row.contains("id=problems") && row.contains("label=PROBLEMS")
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
            && row.contains("id=agent-log")
            && row.contains("label=AGENT LOG")
    }));

    let delegated = DesktopProjectionViewModel::from_snapshot(&populated_snapshot());
    assert!(delegated.autonomy_scale_rows.iter().any(|row| {
        row.contains("label=Delegate")
            && row.contains("active=true")
            && row.contains("confirm=required")
    }));
    assert!(!delegated.command_palette_overlay.open);
    assert!(delegated.bottom_tab_rows.iter().any(|row| {
        row.contains("mode=Delegate")
            && row.contains("id=term")
            && row.contains("label=TERMINAL")
            && row.contains("active=true")
    }));
}

#[test]
fn projection_rendering_models_prototype_workbench_composition() {
    let mut snapshot = populated_snapshot();
    snapshot
        .context_manifest_projection
        .manifest
        .workspace_trust_state = Some(legion_protocol::WorkspaceTrustState::Trusted);
    snapshot.language_tooling_projection.status = legion_protocol::LanguageToolingStatusKind::Ready;
    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert_eq!(
        model.top_bar_rows,
        vec![
            "top bar identity: Legion workspace=Foundation Mode".to_string(),
            "top bar modes: Manual | Assist | Delegate | Legion Workflows active=Delegate"
                .to_string(),
            "top bar command: label=Command presence=1".to_string(),
        ],
        "the first-screen header should expose only identity, mode, and command/presence regions"
    );
    assert_eq!(
        model.left_sidebar_rows,
        vec![
            "explorer chrome: title=EXPLORER · Foundation Mode nodes=2 selected_file=2".to_string()
        ],
        "the first-screen sidebar should be the projected explorer, without fleet or context-pack summaries"
    );
    assert_eq!(model.center_surface, "editor");
    assert!(model.bottom_tab_rows.iter().any(|row| {
        row.contains("id=term") && row.contains("label=TERMINAL") && row.contains("active=true")
    }));
    assert!(model.bottom_tab_rows.iter().any(|row| {
        row.contains("id=problems") && row.contains("label=PROBLEMS") && row.contains("count=0")
    }));
    assert!(
        model
            .bottom_tab_rows
            .iter()
            .any(|row| { row.contains("id=agent-log") && row.contains("label=AGENT LOG") })
    );
    assert_eq!(model.status_bar.trust.as_deref(), Some("Trusted"));
    assert_eq!(
        model.status_bar.lsp, None,
        "general language readiness must not be relabeled as live LSP state"
    );

    snapshot.language_tooling_projection.lsp_session_status =
        Some(legion_protocol::LspSessionStatusProjection {
            lifecycle: legion_protocol::LspSessionLifecycleKind::Live,
            restart_count: 0,
            max_auto_restarts: 3,
            backoff_remaining_ms: None,
            failure_reason: None,
            schema_version: 1,
        });
    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    assert_eq!(model.status_bar.lsp.as_deref(), Some("Live"));
}

#[test]
fn projection_rendering_never_projects_live_agent_log_in_manual() {
    let model =
        DesktopProjectionViewModel::from_snapshot(&Shell::empty("Manual").projection_snapshot());

    assert!(model.bottom_tab_rows.iter().any(|row| {
        row.contains("id=term") && row.contains("label=TERMINAL") && row.contains("active=true")
    }));
    assert!(
        model
            .bottom_tab_rows
            .iter()
            .any(|row| row.contains("id=problems") && row.contains("label=PROBLEMS"))
    );
    assert!(
        model
            .bottom_tab_rows
            .iter()
            .all(|row| !row.contains("AGENT LOG")),
        "Manual must not expose a live-agent console surface"
    );
}

#[test]
fn projection_rendering_suppresses_projected_presence_from_manual_chrome() {
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Manual;

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(
        model
            .top_bar_rows
            .iter()
            .any(|row| row == "top bar command: label=Command presence=0"),
        "Manual chrome must not expose collaboration-presence claims"
    );
}

#[test]
fn projection_rendering_uses_the_canonical_four_mode_switch() {
    let model =
        DesktopProjectionViewModel::from_snapshot(&Shell::empty("Manual").projection_snapshot());
    let expected = [
        ("n=1", "key=M", "label=Manual"),
        ("n=2", "key=A", "label=Assist"),
        ("n=3", "key=D", "label=Delegate"),
        ("n=4", "key=W", "label=Legion Workflows"),
    ];

    assert_eq!(model.autonomy_scale_rows.len(), expected.len());
    for (row, (ordinal, shortcut, label)) in model.autonomy_scale_rows.iter().zip(expected) {
        assert!(row.contains(ordinal), "missing {ordinal} in {row}");
        assert!(row.contains(shortcut), "missing {shortcut} in {row}");
        assert!(row.contains(label), "missing {label} in {row}");
    }
    assert!(
        model
            .product_mode_rows
            .iter()
            .any(|row| row == "product modes: Manual | Assist | Delegate | Legion Workflows")
    );
}

#[test]
fn projection_rendering_uses_stable_responsive_shell_geometry() {
    let desktop = ShellGeometry::for_available_size(1440.0, 900.0);
    assert_eq!(desktop.top_bar_height, 42.0);
    assert_eq!(desktop.activity_rail_width, 46.0);
    assert_eq!(desktop.explorer_width, 248.0);
    assert_eq!(desktop.left_width, 294.0);
    assert_eq!(desktop.right_width, 325.0);
    assert_eq!(desktop.bottom_height, 192.0);
    assert_eq!(desktop.status_bar_height, 24.0);
    assert!(!desktop.compact);

    let compact = ShellGeometry::for_available_size(960.0, 720.0);
    assert!(compact.compact);
    assert_eq!(compact.left_width, 250.0);
    assert_eq!(compact.right_width, 325.0);
    assert!(compact.editor_width(960.0) >= 360.0);
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
                detail: Some("Workspace file".to_string()),
                shortcut_label: Some("Enter".to_string()),
                path: Some("Cargo.toml".to_string()),
                buffer_id: None,
                position: None,
                match_indices: vec![0, 5],
                disabled_reason: None,
            },
            PaletteResult {
                id: "command:save-all".to_string(),
                kind: PaletteResultKind::Command,
                title: "Save All".to_string(),
                detail: Some("Save all open files".to_string()),
                shortcut_label: Some("Ctrl+S".to_string()),
                path: None,
                buffer_id: None,
                position: None,
                match_indices: vec![0, 5],
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
                path: Some(format!("item-{index}.rs")),
                buffer_id: None,
                position: None,
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
        editor_font_family: "  JetBrains Mono<script>\n".to_string(),
        editor_font_size_pt: 8,
        font_fallback_diagnostics: Vec::new(),
        toast_verbosity: ToastVerbosityProjection::All,
        terminal_shell_selection: String::new(),
        editor: legion_ui::EditorSettingsProjection {
            line_numbers_visible: false,
            current_line_highlight: false,
            sticky_headers_visible: false,
            code_folding_visible: true,
            minimap_visible: false,
            whitespace_guides_visible: true,
            indent_guides_visible: true,
            smooth_scrolling_enabled: true,
            line_wrapping_policy: LineWrappingPolicy::FixedColumn,
            wrap_column: Some(12),
        },
        telemetry: legion_protocol::WorkbenchTelemetryConsent::default(),
        indexed_workspace_search_enabled: false,
        next_edit_prediction_enabled: false,
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
    assert_eq!(model.settings.editor_font_family, "JetBrains Monoscript");
    assert_eq!(
        model.settings.toast_verbosity,
        ToastVerbosityProjection::All
    );
    assert_eq!(model.settings.toast_verbosity_label, "All statuses");
    assert!(!model.settings.line_numbers_visible);
    assert!(!model.settings.current_line_highlight);
    assert!(!model.settings.sticky_headers_visible);
    assert!(model.settings.code_folding_visible);
    assert!(!model.settings.minimap_visible);
    assert!(model.settings.whitespace_guides_visible);
    assert!(model.settings.indent_guides_visible);
    assert!(model.settings.smooth_scrolling_enabled);
    assert_eq!(
        model.settings.line_wrapping_policy,
        LineWrappingPolicy::FixedColumn
    );
    assert_eq!(model.settings.wrap_column, Some(40));
    assert_eq!(model.settings.wrapping_row, "wrapping: fixed_column 40");
    assert!(!model.settings.crash_reports_enabled);
    assert_eq!(model.settings.telemetry_label, "local-only");
    assert!(
        model
            .main_canvas_rows
            .iter()
            .any(|row| row.contains("editor polish:"))
    );
    assert_eq!(model.settings.schema_version, 1);
}

#[test]
fn projection_rendering_projects_editor_polish_summary_rows() {
    let mut snapshot = Shell::empty("Editor polish").projection_snapshot();
    snapshot.settings_projection.editor.sticky_headers_visible = true;
    snapshot.settings_projection.editor.code_folding_visible = true;
    snapshot.settings_projection.editor.minimap_visible = true;
    snapshot
        .settings_projection
        .editor
        .whitespace_guides_visible = true;
    snapshot.settings_projection.editor.indent_guides_visible = true;
    snapshot.settings_projection.editor.smooth_scrolling_enabled = false;
    snapshot.language_tooling_projection.sticky_scopes = vec![LanguageStickyScopeProjection {
        scope_id: "scope:fn".to_string(),
        label: "fn render_editor_polish()".to_string(),
        kind_label: "function".to_string(),
        range: None,
        depth: 0,
        active: true,
        source_label: "test".to_string(),
        schema_version: 1,
    }];
    snapshot.active_buffer_projection.viewport = Some(ViewportProjection {
        workspace_id: WorkspaceId(1),
        buffer_id: BufferId(1),
        file_id: None,
        snapshot_id: SnapshotId(1),
        buffer_version: BufferVersion(1),
        visible_range: range(0, 1),
        selections: Vec::new(),
        cursor: coord(0, 0, 0),
        cursors: vec![coord(0, 0, 0)],
        scroll: ViewportScroll {
            top_line: 0,
            left_column: 0,
        },
        dimensions: ViewportDimensions {
            width_px: 80,
            height_px: 24,
        },
        line_wrapping_policy: LineWrappingPolicy::Off,
        wrap_column: None,
        mode: ViewportProjectionMode::Normal,
        line_slices: Vec::new(),
        line_metrics: Vec::new(),
        decoration_spans: Vec::new(),
        fold_ranges: vec![ViewportFoldRange::default()],
        semantic_token_overlays: Vec::new(),
        large_file_status: None,
        schema_version: 1,
    });

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);

    assert!(model.main_canvas_rows.iter().any(|row| {
        row.contains("editor polish:")
            && row.contains("sticky_headers=true")
            && row.contains("code_folding=true")
            && row.contains("minimap=true")
            && row.contains("whitespace_guides=true")
            && row.contains("indent_guides=true")
            && row.contains("smooth_scrolling=false")
            && row.contains("fold_ranges=1")
            && row.contains("sticky_scopes=1")
    }));
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
    // SCALE.05 moved capability reductions into per-reason banner bullet rows
    // ("  • capability reduced: {reason}", view.rs). Assert the disabled
    // overlay reason appears in one of them.
    assert!(
        degraded_model
            .large_file_banner_rows
            .iter()
            .any(|row| row.contains("capability reduced: semantic overlays disabled"))
    );
    assert!(degraded_model.large_file_banner_rows.iter().all(|row| {
        !row.contains("HIDDEN_NEEDLE_AFTER_VIEWPORT")
            && !row.contains("\\Windows\\Fonts")
            && !row.contains("/System/Library/Fonts")
            && !row.contains("/usr/share/fonts")
            && !row.contains('\n')
            && !row.contains('\0')
    }));

    let streaming_model = DesktopProjectionViewModel::from_snapshot(&streaming_snapshot());
    // There is no separate "streaming" buffer state; a chunked viewport renders
    // its line slices directly, each prefixed with the 1-based line number.
    assert_eq!(
        streaming_model.active_buffer_lines,
        vec!["   1: visible streaming line".to_string()]
    );
    assert!(
        streaming_model
            .active_buffer_lines
            .iter()
            .any(|row| row.contains("visible streaming line"))
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
            truncation_state: ViewportLineTruncationState::None,
        },
        DesktopCodeLineViewModel {
            number: 5,
            text: "beta_value".to_string(),
            highlights: Vec::new(),
            truncation_state: ViewportLineTruncationState::None,
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
        truncation_state: ViewportLineTruncationState::None,
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
        truncation_state: ViewportLineTruncationState::None,
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

fn desktop_raw_input_at(size: egui::Vec2, events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        events,
        ..egui::RawInput::default()
    }
}

fn render_projection_frame(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
) -> (ProjectionViewOutput, egui::FullOutput) {
    render_projection_frame_at(ctx, view, snapshot, egui::vec2(1_440.0, 900.0))
}

fn render_projection_frame_at(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
    size: egui::Vec2,
) -> (ProjectionViewOutput, egui::FullOutput) {
    let mut projection_output = None;
    let full_output = ctx.run_ui(desktop_raw_input_at(size, Vec::new()), |ui| {
        projection_output = Some(view.render(ui, snapshot));
    });
    (
        projection_output.expect("projection view should render"),
        full_output,
    )
}

fn accesskit_bounds(
    output: &egui::FullOutput,
    label: &str,
    clickable: bool,
) -> egui::accesskit::Rect {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("AccessKit update should be enabled")
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            ((node.label() == Some(label) || node.value() == Some(label))
                && (!clickable || node.supports_action(egui::accesskit::Action::Click)))
            .then(|| node.bounds())
            .flatten()
        })
        .unwrap_or_else(|| panic!("accessible control `{label}` should be allocated"))
}

fn accesskit_button_bounds_in_x_range(
    output: &egui::FullOutput,
    label: &str,
    x_range: std::ops::RangeInclusive<f32>,
) -> egui::accesskit::Rect {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("AccessKit update should be enabled")
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            let bounds = node.bounds()?;
            let center_x = ((bounds.x0 + bounds.x1) * 0.5) as f32;
            (node.label() == Some(label)
                && node.supports_action(egui::accesskit::Action::Click)
                && x_range.contains(&center_x))
            .then_some(bounds)
        })
        .unwrap_or_else(|| panic!("central accessible control `{label}` should be allocated"))
}

fn accesskit_has_label(output: &egui::FullOutput, label: &str) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .is_some_and(|update| {
            update
                .nodes
                .iter()
                .any(|(_id, node)| node.label() == Some(label) || node.value() == Some(label))
        })
}

fn click_accessible_control(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
    primed: &egui::FullOutput,
    label: &str,
) -> (ProjectionViewOutput, egui::FullOutput) {
    click_accessible_control_at(
        ctx,
        view,
        snapshot,
        primed,
        label,
        egui::vec2(1_440.0, 900.0),
    )
}

fn click_accessible_control_at(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
    primed: &egui::FullOutput,
    label: &str,
    size: egui::Vec2,
) -> (ProjectionViewOutput, egui::FullOutput) {
    let bounds = accesskit_bounds(primed, label, true);
    let pos = egui::pos2(
        ((bounds.x0 + bounds.x1) * 0.5) as f32,
        ((bounds.y0 + bounds.y1) * 0.5) as f32,
    );
    click_projection_at(ctx, view, snapshot, pos, size)
}

fn click_projection_at(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
    pos: egui::Pos2,
    size: egui::Vec2,
) -> (ProjectionViewOutput, egui::FullOutput) {
    let press = desktop_raw_input_at(
        size,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            },
        ],
    );
    let _ = ctx.run_ui(press, |ui| {
        let _ = view.render(ui, snapshot);
    });

    let release = desktop_raw_input_at(
        size,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            },
        ],
    );
    let mut projection_output = None;
    let full_output = ctx.run_ui(release, |ui| {
        projection_output = Some(view.render(ui, snapshot));
    });
    (
        projection_output.expect("clicked projection frame should render"),
        full_output,
    )
}

#[test]
fn projection_rendering_bottom_tabs_are_real_controls_with_persistent_renderer_state() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Delegate;

    let (_initial, mut full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert_eq!(view.selected_bottom_panel(), BottomPanelTab::Terminal);
    let terminal = accesskit_bounds(&full, "TERMINAL", true);
    assert!(terminal.y1 - terminal.y0 >= 24.0);
    assert!(accesskit_has_label(&full, "Terminal / Runtime"));

    let (_clicked, next) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "PROBLEMS (0)");
    full = next;
    assert_eq!(view.selected_bottom_panel(), BottomPanelTab::Problems);
    assert!(accesskit_has_label(&full, "Problems"));
    assert!(!accesskit_has_label(&full, "Terminal / Runtime"));
    let (problems_frame, _) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(
        problems_frame
            .bottom_tab_rows
            .iter()
            .any(|row| { row.contains("id=problems") && row.contains("active=true") })
    );
    assert!(
        problems_frame
            .bottom_tab_rows
            .iter()
            .any(|row| { row.contains("id=term") && row.contains("active=false") })
    );

    snapshot.product_mode = DockMode::Assist;
    let (_assist, next) = render_projection_frame(&ctx, &mut view, &snapshot);
    full = next;
    assert_eq!(
        view.selected_bottom_panel(),
        BottomPanelTab::Problems,
        "valid bottom-panel selection should survive mode changes"
    );

    let (_clicked, next) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "AGENT LOG");
    full = next;
    assert_eq!(view.selected_bottom_panel(), BottomPanelTab::AgentLog);
    assert!(accesskit_has_label(&full, "Agent Comm Stream"));
    let (agent_frame, _) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(
        agent_frame
            .bottom_tab_rows
            .iter()
            .any(|row| { row.contains("id=agent-log") && row.contains("active=true") })
    );
    assert!(
        agent_frame
            .bottom_tab_rows
            .iter()
            .any(|row| { row.contains("id=term") && row.contains("active=false") })
    );

    snapshot.product_mode = DockMode::Manual;
    let (_manual, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert_eq!(view.selected_bottom_panel(), BottomPanelTab::Terminal);
    assert!(!accesskit_has_label(&full, "AGENT LOG"));
    assert!(!accesskit_has_label(&full, "Agent Comm Stream"));
    assert!(accesskit_has_label(&full, "Terminal / Runtime"));
}

#[test]
fn projection_rendering_expanded_workbenches_leave_a_usable_visible_editor() {
    for size in [egui::vec2(960.0, 720.0), egui::vec2(1_440.0, 900.0)] {
        for (mode, disclosure, action) in [
            (DockMode::Assist, "Assist workbench", "Predict"),
            (DockMode::Delegate, "Delegate workbench", "Approve"),
            (
                DockMode::Automate,
                "Legion Workflows workbench",
                "Force Review",
            ),
        ] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut view = ProjectionView::new();
            let mut snapshot = populated_snapshot();
            snapshot.product_mode = mode;

            let (_closed, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
            let (_opened, _) =
                click_accessible_control_at(&ctx, &mut view, &snapshot, &full, disclosure, size);
            let mut full = render_projection_frame_at(&ctx, &mut view, &snapshot, size).1;
            for _ in 0..8 {
                full = render_projection_frame_at(&ctx, &mut view, &snapshot, size).1;
            }
            let editor_rect = view
                .last_editor_rect()
                .expect("the real editor surface should record its allocation");
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            let visible_editor = editor_rect.intersect(screen);
            assert!(
                visible_editor.height() >= 180.0,
                "{mode:?} at {size:?} must leave at least 180px of the editor visible; actual editor={editor_rect:?}, visible={visible_editor:?}"
            );

            let action_bounds = accesskit_button_bounds_in_x_range(
                &full,
                action,
                editor_rect.left()..=editor_rect.right(),
            );
            let action_pos = egui::pos2(
                ((action_bounds.x0 + action_bounds.x1) * 0.5) as f32,
                ((action_bounds.y0 + action_bounds.y1) * 0.5) as f32,
            );
            let (action_output, _) =
                click_projection_at(&ctx, &mut view, &snapshot, action_pos, size);
            assert_eq!(
                action_output.actions.len(),
                1,
                "{mode:?} action should remain clickable inside the bounded workbench"
            );
        }
    }
}

#[test]
fn projection_rendering_advanced_disclosure_precedes_editor_and_routes_mode_actions() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Assist;

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let header = accesskit_bounds(&full, "Assist workbench", true);
    let editor = accesskit_bounds(&full, "[workspace]", false);
    assert!(
        header.y1 <= editor.y0,
        "collapsed advanced disclosure must be allocated before the editor scroll surface"
    );

    let (_opened, _) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Assist workbench");
    let (_open_frame, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (assist_action, _) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Predict");
    assert!(matches!(
        assist_action.actions.as_slice(),
        [DesktopAction::RequestAssistInlinePrediction { .. }]
    ));

    snapshot.product_mode = DockMode::Delegate;
    let (_delegate, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (delegate_action, _) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Approve");
    assert_eq!(
        delegate_action.actions,
        vec![DesktopAction::ApproveProposal {
            proposal_id: ProposalId(7)
        }]
    );

    snapshot.product_mode = DockMode::Automate;
    let (_workflows, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (workflow_action, _) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Force Review");
    assert_eq!(
        workflow_action.actions,
        vec![DesktopAction::PreviewProposal {
            proposal_id: ProposalId(7)
        }]
    );
}

#[test]
fn projection_rendering_workbench_toolbox_routes_git_test_and_debug_outside_manual() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Assist;

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (_opened, _) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Workbench tools");
    let (_open_frame, full) = render_projection_frame(&ctx, &mut view, &snapshot);

    let (git, _) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Refresh Git");
    assert_eq!(git.actions, vec![DesktopAction::RefreshGit]);

    let (_refresh, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (tests, _) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Refresh tests");
    assert_eq!(tests.actions, vec![DesktopAction::RefreshTestExplorer]);

    let (_refresh, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (debug, _) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Refresh configs");
    assert_eq!(
        debug.actions,
        vec![DesktopAction::RefreshDebugConfigurations]
    );
}

#[test]
fn projection_rendering_tests_preserve_app_boundary() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/view.rs"))
        .expect("renderer source should be readable");

    common::assert_source_excludes(&source, "src/view.rs", &["legion_app", "AppComposition"]);
}
