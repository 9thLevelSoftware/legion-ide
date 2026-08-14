use std::collections::BTreeSet;

use legion_desktop::bridge::DesktopAction;
use legion_desktop::view::ShellGeometry;
use legion_desktop::view::{
    BottomPanelTab, DELEGATE_TASK_DRAFT_MAX_BYTES, DELEGATE_TASK_DRAFT_MAX_CHARS,
    DesktopCodeHighlightSpan, DesktopCodeLineViewModel, DesktopProjectionViewModel,
    DesktopProjectionViewState, ProjectionView, ProjectionViewOutput,
    desktop_default_delegated_scope, desktop_delegated_task_action, drag_anchor_for_line_pointer,
    drag_selection_range, editor_coordinate_from_pointer, line_range_for_code_line,
    word_range_for_coordinate,
};
use legion_protocol::{
    ApprovalChecklistGateKind, ApprovalChecklistGateStatus, ApprovalChecklistGateSummary,
    ApprovalChecklistReason, ArtifactKind, ArtifactLedgerProjection, ArtifactLedgerRow,
    AssistedAiOperationClass, AssistedAiProviderAvailabilityState,
    AssistedAiProviderCapabilitySummary, AssistedAiProviderClass, BufferId, BufferVersion,
    ByteRange, CanonicalPath, CapabilityId, CollaborationParticipantId,
    CollaborationPresenceProjection, CollaborationSessionId, CommandDescriptor,
    CommandRegistryProjection, CommandRiskLabel, ContextManifestEgressStatus,
    ContextManifestInclusionState, ContextManifestItem, ContextManifestItemCount,
    ContextManifestItemKind, FileFingerprint, FileId, LanguageStickyScopeProjection,
    LargeFileStatus, LegionWorkflowMergeReadiness, LegionWorkflowMergeReadinessBlocker,
    LegionWorkflowMergeReadinessState, LegionWorkflowProjectionRow, LegionWorkflowSessionId,
    LegionWorkflowState, LineWrappingPolicy, PluginCommandDescriptor, PluginContribution,
    PluginContributionProjection, PluginId, PrincipalId, ProposalContextManifestSummary,
    ProposalDiffSummary, ProposalDiffSummaryKind, ProposalId, ProposalLedgerProjection,
    ProposalLedgerRow, ProposalLifecycleState, ProposalLifecycleStateDisplay, ProposalPayloadKind,
    ProposalPrivacyLabel, ProposalRiskLabel, ProposalRollbackAvailability, ProposalTargetCoverage,
    ProposalTargetCoverageKind, ProtocolTextRange, RedactionHint, SemanticPrivacyScope, SnapshotId,
    SystemGraphEdge, SystemGraphNode, SystemGraphProjection, TerminalSessionId, TextCoordinate,
    TimestampMillis, Utf16Position, Utf16Range, VerificationRunProjection, VerificationRunRow,
    VerificationRunState, ViewportDimensions, ViewportFoldRange, ViewportLineSlice,
    ViewportLineTruncationState, ViewportProjection, ViewportProjectionMode, ViewportScroll,
    ViewportSemanticTokenKind, ViewportSemanticTokenOverlay, WorkspaceId,
};
use legion_protocol::{LanguageCodeLensProjection, LanguageOutlineSymbolProjection};
use legion_ui::ui::{
    CloseDirtyPromptProjection, DailyEditingProjection, EditorTabProjection, EditorTabsProjection,
    EditorViewportStateProjection,
};
use legion_ui::{
    ActiveBufferProjection, ActiveBufferProjectionState, AssistInlinePredictionProjection,
    AssistInlinePredictionRowProjection, AssistInlinePredictionStatusProjection, DockMode,
    ExplorerNodeProjection, ExplorerProjection, ExplorerSelectionProjection, PaletteMode,
    PaletteProjection, PaletteResult, PaletteResultKind, SearchProjection, SearchResultProjection,
    SearchScopeProjection, SearchStatusKindProjection, SearchStatusProjection, SettingsProjection,
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

fn available_local_provider() -> AssistedAiProviderCapabilitySummary {
    AssistedAiProviderCapabilitySummary {
        provider_id: "local".to_string(),
        provider_label: "Local fixture".to_string(),
        provider_class: AssistedAiProviderClass::Local,
        supported_operations: vec![AssistedAiOperationClass::ProposeEdit],
        supported_operation_count: 1,
        model_capability_label_count: 1,
        tool_capability_label_count: 0,
        context_window_label: "bounded".to_string(),
        cost_budget_label: "free".to_string(),
        risk_budget_label: "review required".to_string(),
        privacy_retention_label: "local only".to_string(),
        availability: AssistedAiProviderAvailabilityState::Available,
        refusal: None,
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
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
    snapshot.assisted_ai_projection.providers = vec![available_local_provider()];
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
    snapshot.assisted_ai_projection.provider_count = 1;
    snapshot.assisted_ai_projection.providers = vec![available_local_provider()];
    snapshot
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiStateMatrixState {
    Empty,
    Blocked,
    Ready,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiStateMatrixExpectation {
    Text(&'static str),
    Clickable(&'static str),
    Disabled {
        label: &'static str,
        explanation: &'static str,
    },
    DirtyEditor {
        tab_label: &'static str,
        description: &'static str,
    },
}

fn state_matrix_active_buffer(snapshot: &mut legion_ui::ShellProjectionSnapshot, dirty: bool) {
    snapshot.active_buffer_projection = ActiveBufferProjection {
        state: ActiveBufferProjectionState::Full,
        workspace_id: Some(WorkspaceId(1)),
        buffer_id: Some(BufferId(3)),
        file_id: Some(FileId(2)),
        file_path: Some(CanonicalPath("src/lib.rs".to_string())),
        viewport: None,
        degraded: false,
        small_buffer_preview: Some("fn state_matrix() {}".to_string()),
        dirty,
    };
    snapshot.daily_editing_projection.tabs = EditorTabsProjection {
        tabs: vec![EditorTabProjection {
            buffer_id: BufferId(3),
            file_id: Some(FileId(2)),
            file_path: Some(CanonicalPath("src/lib.rs".to_string())),
            title: "src/lib.rs".to_string(),
            active: true,
            dirty,
            pinned: false,
            preview: false,
        }],
        active_buffer_id: Some(BufferId(3)),
    };
}

fn state_matrix_workflow_row(
    lifecycle_state: LegionWorkflowState,
    readiness_state: LegionWorkflowMergeReadinessState,
) -> LegionWorkflowProjectionRow {
    LegionWorkflowProjectionRow {
        session_id: LegionWorkflowSessionId("state-matrix".to_string()),
        directive_artifact_id: None,
        spec_artifact_id: None,
        task_graph_artifact_id: None,
        lifecycle_state,
        worker_count: 1,
        provider_route_required_count: 0,
        dependency_count: 0,
        unresolved_conflict_count: u32::from(
            readiness_state == LegionWorkflowMergeReadinessState::Blocked,
        ),
        verification_gate_count: 1,
        passed_verification_count: u32::from(
            readiness_state != LegionWorkflowMergeReadinessState::Blocked,
        ),
        sign_off_count: 1,
        signed_off_count: u32::from(readiness_state == LegionWorkflowMergeReadinessState::Ready),
        linked_proposals: Vec::new(),
        merge_readiness: LegionWorkflowMergeReadiness {
            state: readiness_state,
            blockers: if readiness_state == LegionWorkflowMergeReadinessState::Blocked {
                vec![LegionWorkflowMergeReadinessBlocker::FailedVerification]
            } else {
                Vec::new()
            },
            labels: vec!["state matrix".to_string()],
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        },
        display_safe_labels: vec!["state matrix".to_string()],
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }
}

fn state_matrix_case(
    mode: DockMode,
    matrix_state: UiStateMatrixState,
) -> (
    legion_ui::ShellProjectionSnapshot,
    DesktopProjectionViewState,
    UiStateMatrixExpectation,
) {
    let mut snapshot = Shell::empty("State matrix").projection_snapshot();
    snapshot.product_mode = mode;
    let mut view_state = DesktopProjectionViewState::default();
    let expectation = match (mode, matrix_state) {
        (DockMode::Manual, UiStateMatrixState::Empty) => {
            UiStateMatrixExpectation::Text("<no active buffer>")
        }
        (DockMode::Manual, UiStateMatrixState::Blocked) => {
            snapshot.active_buffer_projection.workspace_id = Some(WorkspaceId(1));
            UiStateMatrixExpectation::Disabled {
                label: "Save active file",
                explanation: "Open a file to enable saving.",
            }
        }
        (DockMode::Manual, UiStateMatrixState::Ready) => {
            state_matrix_active_buffer(&mut snapshot, false);
            UiStateMatrixExpectation::Text("fn state_matrix() {}")
        }
        (DockMode::Manual, UiStateMatrixState::Active) => {
            state_matrix_active_buffer(&mut snapshot, true);
            UiStateMatrixExpectation::DirtyEditor {
                tab_label: "src/lib.rs",
                description: "Unsaved changes",
            }
        }
        (DockMode::Assist, UiStateMatrixState::Empty) => {
            snapshot.assisted_ai_projection.provider_count = 1;
            snapshot.assisted_ai_projection.providers = vec![available_local_provider()];
            UiStateMatrixExpectation::Text("No predictions yet")
        }
        (DockMode::Assist, UiStateMatrixState::Blocked) => {
            UiStateMatrixExpectation::Text("Choose an AI provider to enable predictions.")
        }
        (DockMode::Assist, UiStateMatrixState::Ready) => {
            state_matrix_active_buffer(&mut snapshot, false);
            snapshot.assisted_ai_projection.provider_count = 1;
            snapshot.assisted_ai_projection.providers = vec![available_local_provider()];
            UiStateMatrixExpectation::Clickable("Predict")
        }
        (DockMode::Assist, UiStateMatrixState::Active) => {
            snapshot = assist_inline_prediction_snapshot();
            UiStateMatrixExpectation::Text(".await")
        }
        (DockMode::Delegate, UiStateMatrixState::Empty) => {
            view_state.canonical_workspace_root =
                Some(CanonicalPath("D:/state-matrix".to_string()));
            UiStateMatrixExpectation::Text("Describe a task to start Delegate.")
        }
        (DockMode::Delegate, UiStateMatrixState::Blocked) => {
            UiStateMatrixExpectation::Text("Open a workspace to define Delegate scope.")
        }
        (DockMode::Delegate, UiStateMatrixState::Ready) => {
            view_state.canonical_workspace_root =
                Some(CanonicalPath("D:/state-matrix".to_string()));
            UiStateMatrixExpectation::Clickable("Delegate task")
        }
        (DockMode::Delegate, UiStateMatrixState::Active) => {
            snapshot.delegated_task_projection.plan_count = 1;
            UiStateMatrixExpectation::Text("Task is active")
        }
        (DockMode::Automate, UiStateMatrixState::Empty) => {
            UiStateMatrixExpectation::Text("No workflow sessions yet")
        }
        (DockMode::Automate, UiStateMatrixState::Blocked) => {
            snapshot.legion_workflow_projection.rows = vec![state_matrix_workflow_row(
                LegionWorkflowState::Blocked,
                LegionWorkflowMergeReadinessState::Blocked,
            )];
            snapshot.legion_workflow_projection.total_session_count = 1;
            UiStateMatrixExpectation::Text("Blocked")
        }
        (DockMode::Automate, UiStateMatrixState::Ready) => {
            snapshot.legion_workflow_projection.rows = vec![state_matrix_workflow_row(
                LegionWorkflowState::Draft,
                LegionWorkflowMergeReadinessState::Ready,
            )];
            snapshot.legion_workflow_projection.total_session_count = 1;
            UiStateMatrixExpectation::Text("Ready for review")
        }
        (DockMode::Automate, UiStateMatrixState::Active) => {
            snapshot.legion_workflow_projection.rows = vec![state_matrix_workflow_row(
                LegionWorkflowState::Executing,
                LegionWorkflowMergeReadinessState::WaitingForApproval,
            )];
            snapshot.legion_workflow_projection.total_session_count = 1;
            UiStateMatrixExpectation::Text("Running")
        }
    };
    (snapshot, view_state, expectation)
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
        row.contains("mode=Assist") && row.contains("id=activity") && row.contains("label=ACTIVITY")
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
            && row.contains("proposal_mediated=true")
            && row.contains("bounded_permissions=true")
            && row.contains("grants_permissions=false")
            && row.contains("security_boundary=false")
    }));
    assert!(manual.mode_confirmation_rows.iter().any(|row| {
        row.contains("target=Legion Workflows")
            && row.contains("required=true")
            && row.contains("proposal_mediated=true")
            && row.contains("grants_permissions=false")
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
        row.contains("mode=Assist") && row.contains("id=activity") && row.contains("label=ACTIVITY")
    }));

    let delegated = DesktopProjectionViewModel::from_snapshot(&populated_snapshot());
    assert!(delegated.autonomy_scale_rows.iter().any(|row| {
        row.contains("label=Delegate")
            && row.contains("active=true")
            && row.contains("confirm=none")
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
            .any(|row| { row.contains("id=activity") && row.contains("label=ACTIVITY") })
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
fn projection_rendering_manual_projects_user_facing_activity_without_live_agent_copy() {
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
            .any(|row| row.contains("id=activity") && row.contains("label=ACTIVITY"))
    );
    assert!(
        model
            .bottom_tab_rows
            .iter()
            .all(|row| !row.contains("AGENT LOG"))
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
    assert_eq!(desktop.right_min_width, 288.0);
    assert_eq!(desktop.right_max_width, 480.0);
    assert_eq!(desktop.bottom_height, 192.0);
    assert_eq!(desktop.status_bar_height, 24.0);
    assert!(!desktop.compact);

    let compact = ShellGeometry::for_available_size(960.0, 720.0);
    assert!(compact.compact);
    assert_eq!(compact.left_width, 0.0);
    assert_eq!(compact.right_width, 0.0);
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
fn projection_rendering_groups_command_palette_results_in_the_product_hierarchy() {
    let mut snapshot = Shell::empty("Command groups").projection_snapshot();
    snapshot.palette_projection = PaletteProjection {
        open: true,
        mode: PaletteMode::Command,
        query: ">".to_string(),
        scope: SearchScopeProjection::Workspace,
        selected_index: 5,
        results: [
            ("refresh-explorer", "Refresh Explorer", None),
            ("save-all", "Save All", None),
            ("preferences-open", "Open Settings", None),
            ("lsp-start-session", "Start Language Server", None),
            ("refresh-git", "Refresh Git", None),
            (
                "git-delete-branch",
                "Delete Git Branch",
                Some("Enter a branch name"),
            ),
        ]
        .into_iter()
        .map(|(id, title, disabled_reason)| PaletteResult {
            id: format!("command:{id}"),
            kind: PaletteResultKind::Command,
            title: title.to_string(),
            detail: None,
            shortcut_label: None,
            path: None,
            buffer_id: None,
            position: None,
            match_indices: Vec::new(),
            disabled_reason: disabled_reason.map(str::to_string),
        })
        .collect(),
    };

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    let rows = &model.command_palette_overlay.result_rows;
    assert_eq!(
        rows.iter()
            .map(|row| row.group_label.as_str())
            .collect::<Vec<_>>(),
        vec!["Suggested", "Files", "View", "Run", "Git", "Destructive"]
    );
    assert!(
        !rows[5].selected,
        "a malformed projection must not visually select an unavailable command"
    );
    assert_eq!(
        rows[5].disabled_reason.as_deref(),
        Some("Enter a branch name")
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

fn render_projection_frame_with_state(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
) -> (ProjectionViewOutput, egui::FullOutput) {
    render_projection_frame_with_state_at(ctx, view, snapshot, state, egui::vec2(1_440.0, 900.0))
}

fn render_projection_frame_with_state_at(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    size: egui::Vec2,
) -> (ProjectionViewOutput, egui::FullOutput) {
    let mut projection_output = None;
    let full_output = ctx.run_ui(desktop_raw_input_at(size, Vec::new()), |ui| {
        projection_output = Some(view.render_with_state(ui, snapshot, state));
    });
    (
        projection_output.expect("projection view with state should render"),
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

fn accesskit_button_bounds_in_y_range(
    output: &egui::FullOutput,
    label: &str,
    y_range: std::ops::RangeInclusive<f32>,
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
            let center_y = ((bounds.y0 + bounds.y1) * 0.5) as f32;
            (node.label() == Some(label)
                && node.supports_action(egui::accesskit::Action::Click)
                && y_range.contains(&center_y))
            .then_some(bounds)
        })
        .unwrap_or_else(|| panic!("top-bar control `{label}` should be allocated"))
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

fn accesskit_contains_text(output: &egui::FullOutput, text: &str) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .is_some_and(|update| {
            update.nodes.iter().any(|(_id, node)| {
                node.label().is_some_and(|label| label.contains(text))
                    || node.value().is_some_and(|value| value.contains(text))
            })
        })
}

fn accesskit_contains_text_in_bounds(
    output: &egui::FullOutput,
    text: &str,
    scope: egui::accesskit::Rect,
) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .is_some_and(|update| {
            update.nodes.iter().any(|(_id, node)| {
                let Some(bounds) = node.bounds() else {
                    return false;
                };
                let center_x = (bounds.x0 + bounds.x1) * 0.5;
                let center_y = (bounds.y0 + bounds.y1) * 0.5;
                center_x >= scope.x0
                    && center_x <= scope.x1
                    && center_y >= scope.y0
                    && center_y <= scope.y1
                    && (node.label().is_some_and(|label| label.contains(text))
                        || node.value().is_some_and(|value| value.contains(text)))
            })
        })
}

fn accesskit_clickable_label_in_bounds(
    output: &egui::FullOutput,
    label: &str,
    scope: egui::accesskit::Rect,
) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .is_some_and(|update| {
            update.nodes.iter().any(|(_id, node)| {
                let Some(bounds) = node.bounds() else {
                    return false;
                };
                let center_x = (bounds.x0 + bounds.x1) * 0.5;
                let center_y = (bounds.y0 + bounds.y1) * 0.5;
                center_x >= scope.x0
                    && center_x <= scope.x1
                    && center_y >= scope.y0
                    && center_y <= scope.y1
                    && node.label() == Some(label)
                    && node.supports_action(egui::accesskit::Action::Click)
            })
        })
}

fn accesskit_largest_label_bounds(
    output: &egui::FullOutput,
    label: &str,
) -> Option<egui::accesskit::Rect> {
    output
        .platform_output
        .accesskit_update
        .as_ref()?
        .nodes
        .iter()
        .filter_map(|(_id, node)| {
            (node.label() == Some(label))
                .then(|| node.bounds())
                .flatten()
        })
        .max_by(|left, right| {
            let left_area = (left.x1 - left.x0) * (left.y1 - left.y0);
            let right_area = (right.x1 - right.x0) * (right.y1 - right.y0);
            left_area.total_cmp(&right_area)
        })
}

fn accesskit_label_has_description(
    output: &egui::FullOutput,
    label: &str,
    description: &str,
) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .is_some_and(|update| {
            update.nodes.iter().any(|(_id, node)| {
                node.label() == Some(label) && node.description() == Some(description)
            })
        })
}

fn accesskit_label_count(output: &egui::FullOutput, label: &str) -> usize {
    let mut bounds = output
        .platform_output
        .accesskit_update
        .as_ref()
        .map_or_else(Vec::new, |update| {
            update
                .nodes
                .iter()
                .filter(|(_id, node)| node.label() == Some(label) || node.value() == Some(label))
                .filter_map(|(_id, node)| node.bounds())
                .collect::<Vec<_>>()
        });
    bounds.dedup_by(|left, right| {
        (left.x0 - right.x0).abs() < 0.5
            && (left.y0 - right.y0).abs() < 0.5
            && (left.x1 - right.x1).abs() < 0.5
            && (left.y1 - right.y1).abs() < 0.5
    });
    bounds.len()
}

fn accesskit_has_clickable_label(output: &egui::FullOutput, label: &str) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .is_some_and(|update| {
            update.nodes.iter().any(|(_id, node)| {
                node.label() == Some(label) && node.supports_action(egui::accesskit::Action::Click)
            })
        })
}

fn accesskit_has_role(output: &egui::FullOutput, role: egui::accesskit::Role) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .is_some_and(|update| update.nodes.iter().any(|(_id, node)| node.role() == role))
}

fn accesskit_label_is_disabled(output: &egui::FullOutput, label: &str) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .is_some_and(|update| {
            update
                .nodes
                .iter()
                .any(|(_id, node)| node.label() == Some(label) && node.is_disabled())
        })
}

fn accesskit_contains_text_in_x_range(
    output: &egui::FullOutput,
    text: &str,
    x_range: std::ops::RangeInclusive<f32>,
) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .is_some_and(|update| {
            update.nodes.iter().any(|(_id, node)| {
                let Some(bounds) = node.bounds() else {
                    return false;
                };
                let center_x = ((bounds.x0 + bounds.x1) * 0.5) as f32;
                x_range.contains(&center_x)
                    && (node.label().is_some_and(|label| label.contains(text))
                        || node.value().is_some_and(|value| value.contains(text)))
            })
        })
}

fn accesskit_dialog_text(output: &egui::FullOutput, dialog_label: &str) -> Vec<String> {
    let update = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("dialog should expose AccessKit");
    let dialog = update
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            (node.role() == egui::accesskit::Role::Dialog && node.label() == Some(dialog_label))
                .then_some(node)
        })
        .unwrap_or_else(|| panic!("dialog `{dialog_label}` should be present"));
    let mut pending = dialog.children().to_vec();
    let mut text = Vec::new();
    while let Some(id) = pending.pop() {
        let node = update
            .nodes
            .iter()
            .find_map(|(candidate, node)| (*candidate == id).then_some(node))
            .unwrap_or_else(|| panic!("dialog descendant {id:?} should be present"));
        if let Some(label) = node.label() {
            text.push(label.to_string());
        }
        if let Some(value) = node.value() {
            text.push(value.to_string());
        }
        pending.extend(node.children().iter().copied());
    }
    text
}

fn accesskit_focused_label(output: &egui::FullOutput) -> Option<&str> {
    let update = output.platform_output.accesskit_update.as_ref()?;
    update
        .nodes
        .iter()
        .find_map(|(id, node)| (*id == update.focus).then(|| node.label()).flatten())
}

fn drag_projection_at(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
    from: egui::Pos2,
    to: egui::Pos2,
    size: egui::Vec2,
) {
    let press = desktop_raw_input_at(
        size,
        vec![
            egui::Event::PointerMoved(from),
            egui::Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            },
        ],
    );
    let _ = ctx.run_ui(press, |ui| {
        let _ = view.render(ui, snapshot);
    });
    let drag = desktop_raw_input_at(size, vec![egui::Event::PointerMoved(to)]);
    let _ = ctx.run_ui(drag, |ui| {
        let _ = view.render(ui, snapshot);
    });
    let release = desktop_raw_input_at(
        size,
        vec![egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        }],
    );
    let _ = ctx.run_ui(release, |ui| {
        let _ = view.render(ui, snapshot);
    });
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

fn click_accessible_control_with_state(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    primed: &egui::FullOutput,
    label: &str,
) -> (ProjectionViewOutput, egui::FullOutput) {
    click_accessible_control_with_state_at(
        ctx,
        view,
        snapshot,
        state,
        primed,
        label,
        egui::vec2(1_440.0, 900.0),
    )
}

fn click_accessible_control_with_state_at(
    ctx: &egui::Context,
    view: &mut ProjectionView,
    snapshot: &legion_ui::ShellProjectionSnapshot,
    state: &DesktopProjectionViewState,
    primed: &egui::FullOutput,
    label: &str,
    size: egui::Vec2,
) -> (ProjectionViewOutput, egui::FullOutput) {
    let bounds = accesskit_bounds(primed, label, true);
    let pos = egui::pos2(
        ((bounds.x0 + bounds.x1) * 0.5) as f32,
        ((bounds.y0 + bounds.y1) * 0.5) as f32,
    );
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
        let _ = view.render_with_state(ui, snapshot, state);
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
    let mut output = None;
    let full = ctx.run_ui(release, |ui| {
        output = Some(view.render_with_state(ui, snapshot, state));
    });
    (
        output.expect("stateful clicked projection frame should render"),
        full,
    )
}

fn seed_delegate_task_draft(ctx: &egui::Context, draft: &str) {
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new("legion-delegate-task-draft-value"),
            draft.to_string(),
        );
    });
}

#[test]
fn projection_rendering_manual_uses_top_mode_switch_without_context_inspector() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Manual;

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(
        view.last_shell_panel_rects()
            .is_some_and(|rects| rects.right.width() <= 2.0)
    );
    assert!(!accesskit_has_label(&full, "AI engine disengaged"));
    assert!(!accesskit_has_label(&full, "Delegation Console"));
    assert!(!accesskit_has_label(&full, "Legion Workflow Control"));
    assert!(!accesskit_has_label(&full, "Current File"));

    let (clicked, _) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Assist");
    assert_eq!(
        clicked.actions,
        vec![DesktopAction::SetProductMode {
            mode: DockMode::Assist
        }]
    );
}

#[test]
fn projection_rendering_manual_first_run_uses_setup_overlay_not_a_context_rail() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Manual;
    let state = DesktopProjectionViewState {
        first_run_onboarding_visible: true,
        ..DesktopProjectionViewState::default()
    };

    let (_initial, full) = render_projection_frame_with_state(&ctx, &mut view, &snapshot, &state);
    assert!(
        view.last_shell_panel_rects()
            .is_some_and(|rects| rects.right.width() <= 2.0)
    );
    assert!(!accesskit_has_role(&full, egui::accesskit::Role::Dialog));
    assert!(accesskit_has_clickable_label(&full, "Setup"));
    assert!(!accesskit_contains_text_in_x_range(
        &full,
        "First-run onboarding",
        1_115.0..=1_440.0
    ));
    let (_opened, full) =
        click_accessible_control_with_state(&ctx, &mut view, &snapshot, &state, &full, "Setup");
    assert!(accesskit_has_role(&full, egui::accesskit::Role::Dialog));
    assert!(accesskit_has_label(&full, "Welcome to Legion"));
}

#[test]
fn projection_rendering_routes_every_mode_pair_through_the_named_confirmation_policy() {
    let cases = [
        (DockMode::Manual, DockMode::Manual, None, false),
        (
            DockMode::Manual,
            DockMode::Assist,
            Some(DockMode::Assist),
            false,
        ),
        (DockMode::Manual, DockMode::Delegate, None, true),
        (DockMode::Manual, DockMode::Automate, None, true),
        (
            DockMode::Assist,
            DockMode::Manual,
            Some(DockMode::Manual),
            false,
        ),
        (DockMode::Assist, DockMode::Assist, None, false),
        (DockMode::Assist, DockMode::Delegate, None, true),
        (DockMode::Assist, DockMode::Automate, None, true),
        (
            DockMode::Delegate,
            DockMode::Manual,
            Some(DockMode::Manual),
            false,
        ),
        (
            DockMode::Delegate,
            DockMode::Assist,
            Some(DockMode::Assist),
            false,
        ),
        (DockMode::Delegate, DockMode::Delegate, None, false),
        (DockMode::Delegate, DockMode::Automate, None, true),
        (
            DockMode::Automate,
            DockMode::Manual,
            Some(DockMode::Manual),
            false,
        ),
        (
            DockMode::Automate,
            DockMode::Assist,
            Some(DockMode::Assist),
            false,
        ),
        (
            DockMode::Automate,
            DockMode::Delegate,
            Some(DockMode::Delegate),
            false,
        ),
        (DockMode::Automate, DockMode::Automate, None, false),
    ];

    for (from, target, expected_action, expected_dialog) in cases {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut view = ProjectionView::new();
        let mut snapshot = Shell::empty("Mode policy").projection_snapshot();
        snapshot.product_mode = from;

        let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
        let target_label = target.label();
        let (clicked, clicked_full) =
            click_accessible_control(&ctx, &mut view, &snapshot, &full, target_label);

        let expected_actions = expected_action
            .map(|mode| vec![DesktopAction::SetProductMode { mode }])
            .unwrap_or_default();
        assert_eq!(
            clicked.actions, expected_actions,
            "unexpected renderer action for {from:?} -> {target:?}"
        );
        assert_eq!(
            accesskit_has_role(&clicked_full, egui::accesskit::Role::Dialog),
            expected_dialog,
            "unexpected confirmation presentation for {from:?} -> {target:?}"
        );
    }
}

#[test]
fn projection_rendering_cancel_confirm_and_snapshot_normalization_preserve_app_authority() {
    let mut snapshot = Shell::empty("Mode confirmation").projection_snapshot();
    snapshot.product_mode = DockMode::Manual;

    let cancel_ctx = egui::Context::default();
    cancel_ctx.enable_accesskit();
    let mut cancel_view = ProjectionView::new();
    let (_initial, full) = render_projection_frame(&cancel_ctx, &mut cancel_view, &snapshot);
    let (opened, modal_full) =
        click_accessible_control(&cancel_ctx, &mut cancel_view, &snapshot, &full, "Delegate");
    assert!(opened.actions.is_empty());
    assert!(accesskit_has_role(
        &modal_full,
        egui::accesskit::Role::Dialog
    ));
    let (settled, modal_full) = render_projection_frame(&cancel_ctx, &mut cancel_view, &snapshot);
    assert!(settled.actions.is_empty());
    let (cancelled, _cancelled_full) = click_accessible_control(
        &cancel_ctx,
        &mut cancel_view,
        &snapshot,
        &modal_full,
        "Cancel",
    );
    assert!(cancelled.actions.is_empty());
    let (_next, cancelled_full) = render_projection_frame(&cancel_ctx, &mut cancel_view, &snapshot);
    assert!(!accesskit_has_role(
        &cancelled_full,
        egui::accesskit::Role::Dialog
    ));

    let confirm_ctx = egui::Context::default();
    confirm_ctx.enable_accesskit();
    let mut confirm_view = ProjectionView::new();
    let (_initial, full) = render_projection_frame(&confirm_ctx, &mut confirm_view, &snapshot);
    let (_opened, _modal_full) = click_accessible_control(
        &confirm_ctx,
        &mut confirm_view,
        &snapshot,
        &full,
        "Delegate",
    );
    let (_settled, modal_full) =
        render_projection_frame(&confirm_ctx, &mut confirm_view, &snapshot);
    let (confirmed, _) = click_accessible_control(
        &confirm_ctx,
        &mut confirm_view,
        &snapshot,
        &modal_full,
        "Confirm",
    );
    assert_eq!(
        confirmed.actions,
        vec![DesktopAction::SetProductMode {
            mode: DockMode::Delegate
        }],
        "confirmation alone should emit exactly one existing product-mode action"
    );

    let stale_ctx = egui::Context::default();
    stale_ctx.enable_accesskit();
    let mut stale_view = ProjectionView::new();
    let (_initial, full) = render_projection_frame(&stale_ctx, &mut stale_view, &snapshot);
    let (_opened, _modal_full) = click_accessible_control(
        &stale_ctx,
        &mut stale_view,
        &snapshot,
        &full,
        "Legion Workflows",
    );
    let (_settled, modal_full) = render_projection_frame(&stale_ctx, &mut stale_view, &snapshot);
    assert!(accesskit_has_role(
        &modal_full,
        egui::accesskit::Role::Dialog
    ));
    snapshot.product_mode = DockMode::Assist;
    let (normalized, normalized_full) =
        render_projection_frame(&stale_ctx, &mut stale_view, &snapshot);
    assert!(normalized.actions.is_empty());
    assert!(
        !accesskit_has_role(&normalized_full, egui::accesskit::Role::Dialog),
        "a changed app snapshot must invalidate renderer-local pending presentation"
    );
}

#[test]
fn projection_rendering_setup_overlay_does_not_duplicate_mode_escalation_controls() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = Shell::empty("Onboarding mode policy").projection_snapshot();
    snapshot.product_mode = DockMode::Assist;
    let state = DesktopProjectionViewState {
        first_run_onboarding_visible: true,
        ..DesktopProjectionViewState::default()
    };

    let (_initial, full) = render_projection_frame_with_state(&ctx, &mut view, &snapshot, &state);
    let (_opened, _full) =
        click_accessible_control_with_state(&ctx, &mut view, &snapshot, &state, &full, "Setup");
    let (_settled, full) = render_projection_frame_with_state(&ctx, &mut view, &snapshot, &state);
    let setup_delegate = full
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("setup should expose AccessKit")
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            let bounds = node.bounds()?;
            (node.label() == Some("Delegate")
                && node.role() == egui::accesskit::Role::Button
                && node.supports_action(egui::accesskit::Action::Click)
                && bounds.y0 > 72.0)
                .then_some(bounds)
        });
    assert!(
        setup_delegate.is_none(),
        "Setup must leave mode escalation on the shared top-bar confirmation path"
    );
    assert!(accesskit_has_role(&full, egui::accesskit::Role::Dialog));
}

#[test]
fn projection_rendering_assist_active_prediction_routes_accept_and_dismiss() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = assist_inline_prediction_snapshot();

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(accesskit_has_label(&full, "Inline prediction"));
    assert!(accesskit_has_label(&full, "Context"));
    assert!(accesskit_has_label(&full, ".await"));
    assert!(!accesskit_has_label(&full, "Predict"));
    assert!(!accesskit_has_label(&full, "Assist workbench"));
    assert!(!accesskit_has_label(&full, "Model Picker"));
    assert!(!accesskit_contains_text_in_x_range(
        &full,
        "Preferred route:",
        1_115.0..=1_440.0
    ));
    assert!(!accesskit_contains_text_in_x_range(
        &full,
        "Anthropic BYOK",
        1_115.0..=1_440.0
    ));
    let (accepted, _) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Accept");
    assert_eq!(
        accepted.actions,
        vec![DesktopAction::AcceptCurrentAssistInlinePrediction]
    );

    let (_next, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (dismissed, _) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Dismiss");
    assert_eq!(
        dismissed.actions,
        vec![DesktopAction::DismissCurrentAssistInlinePrediction]
    );
}

#[test]
fn projection_rendering_assist_idle_and_in_flight_controls_route_existing_actions() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = assist_inline_prediction_snapshot();
    snapshot
        .assist_inline_prediction_projection
        .active_prediction = None;

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (predicted, _) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Predict");
    assert!(matches!(
        predicted.actions.as_slice(),
        [DesktopAction::RequestAssistInlinePrediction { .. }]
    ));

    snapshot
        .assist_inline_prediction_projection
        .request_in_flight = true;
    let (_next, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(accesskit_has_label(&full, "Cancel"));
    assert!(!accesskit_has_label(&full, "Predict"));
    assert!(!accesskit_has_clickable_label(&full, "Predict"));
    let (cancelled, _) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Cancel");
    assert_eq!(
        cancelled.actions,
        vec![DesktopAction::CancelAssistInlinePrediction]
    );
}

#[test]
fn projection_rendering_assist_richer_surfaces_are_read_only_and_never_emit_generation_actions() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Assist;
    snapshot.assisted_ai_projection.preview_ready_count = 1;

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(!accesskit_has_label(&full, "Advanced rail surfaces"));
    for label in ["Suggested Fixes", "Explain This Function", "Generate Test"] {
        assert!(!accesskit_has_label(&full, label));
    }

    assert!(
        !accesskit_has_label(&full, "Assist workbench"),
        "Assist must keep prediction and context status in one inspector surface"
    );
    for label in ["/explain", "/fix", "/test", "/doc"] {
        assert!(!accesskit_has_label(&full, label));
    }
}

#[test]
fn projection_rendering_assist_lists_only_projected_next_edit_rows() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = assist_inline_prediction_snapshot();
    let mut next_edit = snapshot
        .assist_inline_prediction_projection
        .active_prediction
        .clone()
        .expect("active fixture prediction");
    next_edit.prediction_id = "assist:prediction:next-edit".to_string();
    next_edit.ghost_text_label = "Update proposal.rs:74".to_string();
    snapshot.assist_inline_prediction_projection.rows = vec![next_edit];

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(accesskit_has_label(&full, "Next-edit predictions"));
    assert!(accesskit_has_label(&full, "Update proposal.rs:74"));
    assert!(!accesskit_has_label(&full, "Add Autonomous arm"));
}

#[test]
fn projection_rendering_assist_empty_predictions_do_not_invent_suggestion_rows() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = assist_inline_prediction_snapshot();
    snapshot.assist_inline_prediction_projection = AssistInlinePredictionProjection::default();

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(accesskit_has_label(&full, "No predictions yet"));
    assert!(!accesskit_contains_text_in_x_range(
        &full,
        "projected",
        1_115.0..=1_440.0
    ));
    for invented in [
        "Refactor validation into helper",
        "Add null-check for selected value",
        "Generate unit test",
    ] {
        assert!(!accesskit_has_label(&full, invented));
    }
}

#[test]
fn projection_rendering_blank_delegate_draft_is_semantically_disabled() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = Shell::empty("Idle Delegate").projection_snapshot();
    snapshot.product_mode = DockMode::Delegate;

    let state = DesktopProjectionViewState {
        canonical_workspace_root: Some(CanonicalPath("D:/workspace".to_string())),
        ..DesktopProjectionViewState::default()
    };
    let (_initial, full) = render_projection_frame_with_state(&ctx, &mut view, &snapshot, &state);
    assert!(accesskit_label_is_disabled(&full, "Delegate task"));
    assert!(!accesskit_has_clickable_label(&full, "Delegate task"));
    assert!(accesskit_has_label(&full, "Readiness"));
    assert!(accesskit_has_label(&full, "Ready to delegate"));
    assert!(accesskit_has_label(
        &full,
        "Describe a task to start Delegate."
    ));
    assert!(!accesskit_has_label(&full, "Delegate workbench"));
    assert!(!accesskit_has_label(&full, "Inline prediction"));

    seed_delegate_task_draft(&ctx, "   ");
    let (_persisted, full) = render_projection_frame_with_state(&ctx, &mut view, &snapshot, &state);
    assert!(accesskit_label_is_disabled(&full, "Delegate task"));
    assert!(!accesskit_has_clickable_label(&full, "Delegate task"));
    assert_eq!(
        accesskit_label_count(&full, "Describe a task to start Delegate."),
        1
    );
}

#[test]
fn projection_rendering_global_ledgers_do_not_activate_delegate_task_surface() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let populated = populated_snapshot();
    let mut snapshot = Shell::empty("Idle Delegate with global activity").projection_snapshot();
    snapshot.product_mode = DockMode::Delegate;
    snapshot.proposal_ledger_projection = populated.proposal_ledger_projection;
    snapshot.artifact_ledger_projection = populated.artifact_ledger_projection;
    snapshot.verification_run_projection = populated.verification_run_projection;
    let state = DesktopProjectionViewState {
        canonical_workspace_root: Some(CanonicalPath("D:/workspace".to_string())),
        ..DesktopProjectionViewState::default()
    };
    let mut view = ProjectionView::new();

    let (_initial, full) = render_projection_frame_with_state(&ctx, &mut view, &snapshot, &state);

    assert!(accesskit_has_label(&full, "Task description"));
    assert!(accesskit_has_label(&full, "Ready to delegate"));
    assert!(!accesskit_has_label(&full, "Task is active"));
    assert!(!accesskit_has_label(&full, "Delegate workbench"));
    assert!(!accesskit_has_label(&full, "Proposal review"));
    assert!(!accesskit_has_label(&full, "Task graph and evidence"));
}

#[test]
fn projection_rendering_delegate_owned_runtime_and_task_rows_activate_real_console() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();

    let mut runtime_only = Shell::empty("Runtime Delegate").projection_snapshot();
    runtime_only.product_mode = DockMode::Delegate;
    runtime_only.delegated_task_projection.runtime_activation =
        legion_protocol::DelegatedTaskRuntimeActivationState::Verifying;
    let mut view = ProjectionView::new();
    let (_initial, full) = render_projection_frame(&ctx, &mut view, &runtime_only);
    assert!(accesskit_has_label(&full, "Phase"));
    assert!(accesskit_has_label(&full, "Readiness"));
    assert!(accesskit_has_label(&full, "Task is active"));
    assert!(!accesskit_has_label(&full, "Task description"));
    assert!(accesskit_has_label(&full, "Delegate workbench"));
    assert!(!accesskit_has_label(&full, "Inline prediction"));
    let (cancelled, _) =
        click_accessible_control(&ctx, &mut view, &runtime_only, &full, "Cancel task");
    assert_eq!(cancelled.actions, vec![DesktopAction::CancelDelegatedTask]);

    let mut task_owned = Shell::empty("Projected Delegate plan").projection_snapshot();
    task_owned.product_mode = DockMode::Delegate;
    task_owned.delegated_task_projection.plan_count = 1;
    let mut view = ProjectionView::new();
    let (_initial, full) = render_projection_frame(&ctx, &mut view, &task_owned);
    assert!(accesskit_has_label(&full, "Task is active"));
    assert!(accesskit_has_label(&full, "Delegate workbench"));
    assert!(!accesskit_has_label(&full, "Task description"));
}

#[test]
fn projection_rendering_delegate_draft_routes_real_scoped_task_action() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = Shell::empty("Draft Delegate").projection_snapshot();
    snapshot.product_mode = DockMode::Delegate;
    let state = DesktopProjectionViewState {
        canonical_workspace_root: Some(CanonicalPath("D:/workspace".to_string())),
        ..DesktopProjectionViewState::default()
    };
    let expected_scope = desktop_default_delegated_scope(&state)
        .expect("projected workspace root should produce a Delegate scope");
    assert_eq!(
        expected_scope.workspace_root,
        CanonicalPath("D:/workspace".to_string())
    );

    seed_delegate_task_draft(&ctx, "Fix the delegated task rail");
    let (_initial, full) = render_projection_frame_with_state(&ctx, &mut view, &snapshot, &state);
    assert!(accesskit_has_label(&full, "Task description"));
    let (_persisted, full) = render_projection_frame_with_state(&ctx, &mut view, &snapshot, &state);
    let cta = accesskit_bounds(&full, "Delegate task", true);
    assert!(cta.y1 - cta.y0 >= 24.0);
    let delegated = desktop_delegated_task_action(&state, " Fix the delegated task rail ")
        .expect("non-empty draft should create one delegated task action");
    assert_eq!(
        delegated,
        DesktopAction::StartDelegatedTask {
            task_description: "Fix the delegated task rail".to_string(),
            scope: expected_scope,
        }
    );
    assert!(
        !matches!(delegated, DesktopAction::StartAiProposal { .. }),
        "Delegate CTA mapping must never use the proposal-only action"
    );
    assert_eq!(desktop_delegated_task_action(&state, "   "), None);
}

#[test]
fn projection_rendering_delegate_draft_fails_closed_without_projected_workspace_root() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = Shell::empty("Unscoped Delegate").projection_snapshot();
    snapshot.product_mode = DockMode::Delegate;
    let state = DesktopProjectionViewState::default();

    assert_eq!(desktop_default_delegated_scope(&state), None);
    assert_eq!(
        desktop_delegated_task_action(&state, "Do not dispatch without a root"),
        None
    );
    seed_delegate_task_draft(&ctx, "Do not dispatch without a root");
    let (_frame, full) = render_projection_frame_with_state(&ctx, &mut view, &snapshot, &state);
    assert!(accesskit_label_is_disabled(&full, "Delegate task"));
    assert!(accesskit_has_label(
        &full,
        "Open a workspace to define Delegate scope."
    ));
    assert!(accesskit_has_label(
        &full,
        "Open a trusted workspace, then try again."
    ));
    assert!(!accesskit_has_label(&full, "Delegate workbench"));
    assert!(!accesskit_has_label(&full, "Inline prediction"));
}

#[test]
fn projection_rendering_delegate_draft_is_bounded_before_dispatch_on_utf8_boundaries() {
    let state = DesktopProjectionViewState {
        canonical_workspace_root: Some(CanonicalPath("D:/workspace".to_string())),
        ..DesktopProjectionViewState::default()
    };
    let oversized = "🦀".repeat(DELEGATE_TASK_DRAFT_MAX_CHARS + 100);
    let action = desktop_delegated_task_action(&state, &oversized)
        .expect("a non-empty bounded draft should dispatch");
    let DesktopAction::StartDelegatedTask {
        task_description, ..
    } = action
    else {
        panic!("Delegate draft must route to StartDelegatedTask");
    };
    assert_eq!(
        task_description.chars().count(),
        DELEGATE_TASK_DRAFT_MAX_CHARS
    );
    assert!(task_description.len() <= DELEGATE_TASK_DRAFT_MAX_BYTES);
    assert!(task_description.is_char_boundary(task_description.len()));
}

#[test]
fn projection_rendering_delegate_scope_uses_projected_workspace_root_not_nested_manifest() {
    let temp = tempfile::tempdir().expect("temporary workspace should be created");
    let nested = temp.path().join("crates").join("nested");
    std::fs::create_dir_all(&nested).expect("nested crate directory should be created");
    std::fs::write(
        nested.join("Cargo.toml"),
        "[package]\nname='nested'\nversion='0.1.0'\n",
    )
    .expect("nested manifest should be written");
    let workspace_root = CanonicalPath(temp.path().to_string_lossy().into_owned());
    let state = DesktopProjectionViewState {
        canonical_workspace_root: Some(workspace_root.clone()),
        ..DesktopProjectionViewState::default()
    };

    assert_eq!(
        desktop_default_delegated_scope(&state)
            .expect("projected workspace root should produce a scope")
            .workspace_root,
        workspace_root,
        "Delegate scope must come from the runtime projection and never probe for the nearest Cargo.toml"
    );
}

#[test]
fn projection_rendering_visible_actions_meet_minimum_target_height() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = assist_inline_prediction_snapshot();
    snapshot
        .assist_inline_prediction_projection
        .active_prediction = None;

    let (_initial, full) =
        render_projection_frame_at(&ctx, &mut view, &snapshot, egui::vec2(960.0, 720.0));
    let (_drawer, _full) = click_accessible_control_at(
        &ctx,
        &mut view,
        &snapshot,
        &full,
        "Inspector drawer",
        egui::vec2(960.0, 720.0),
    );
    let (_settled, full) =
        render_projection_frame_at(&ctx, &mut view, &snapshot, egui::vec2(960.0, 720.0));
    let predict = accesskit_bounds(&full, "Predict", true);
    assert!(predict.y1 - predict.y0 >= 24.0);

    snapshot.product_mode = DockMode::Manual;
    let mut manual_view = ProjectionView::new();
    let (_manual, full) =
        render_projection_frame_at(&ctx, &mut manual_view, &snapshot, egui::vec2(960.0, 720.0));
    let (_drawer, _full) = click_accessible_control_at(
        &ctx,
        &mut manual_view,
        &snapshot,
        &full,
        "Explorer drawer",
        egui::vec2(960.0, 720.0),
    );
    let (_settled, full) =
        render_projection_frame_at(&ctx, &mut manual_view, &snapshot, egui::vec2(960.0, 720.0));
    for label in ["Settings", "Setup", "Diagnostics"] {
        let bounds = accesskit_bounds(&full, label, true);
        assert!(
            bounds.y1 - bounds.y0 >= 24.0,
            "{label} must retain a >=24px target in the compact viewport; bounds={bounds:?}"
        );
    }
}

#[test]
fn projection_rendering_compact_activity_labels_keep_full_accessible_names() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = Shell::empty("Activity rail").projection_snapshot();

    let (_initial, full) =
        render_projection_frame_at(&ctx, &mut view, &snapshot, egui::vec2(960.0, 720.0));
    let (_opened, _full) = click_accessible_control_at(
        &ctx,
        &mut view,
        &snapshot,
        &full,
        "Explorer drawer",
        egui::vec2(960.0, 720.0),
    );
    let (_settled, full) =
        render_projection_frame_at(&ctx, &mut view, &snapshot, egui::vec2(960.0, 720.0));
    for label in ["Explorer", "Search", "Symbols"] {
        let bounds = accesskit_button_bounds_in_x_range(&full, label, 0.0..=46.0);
        assert!(bounds.x1 - bounds.x0 <= 38.0, "{label}: {bounds:?}");
        assert!(bounds.y1 - bounds.y0 >= 24.0, "{label}: {bounds:?}");
    }

    let search = accesskit_button_bounds_in_x_range(&full, "Search", 0.0..=46.0);
    let (searched, _) = click_projection_at(
        &ctx,
        &mut view,
        &snapshot,
        egui::pos2(
            ((search.x0 + search.x1) * 0.5) as f32,
            ((search.y0 + search.y1) * 0.5) as f32,
        ),
        egui::vec2(960.0, 720.0),
    );
    assert_eq!(
        searched.actions,
        vec![DesktopAction::OpenPalette {
            mode: PaletteMode::Search,
            query: "/".to_string(),
            scope: SearchScopeProjection::ActiveFile,
        }]
    );
}

#[test]
fn projection_rendering_settings_selectors_emit_real_actions() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = Shell::empty("Settings actions").projection_snapshot();
    snapshot.product_mode = DockMode::Assist;

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (_opened, _full) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Settings");
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (theme, _) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Light");
    assert_eq!(
        theme.actions,
        vec![DesktopAction::SetThemePreference {
            preference: ThemePreferenceProjection::Light,
        }]
    );

    let (_next, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (_notifications, _full) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Notifications");
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (toasts, _) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "All statuses");
    assert_eq!(
        toasts.actions,
        vec![DesktopAction::SetToastVerbosity {
            verbosity: ToastVerbosityProjection::All,
        }]
    );
}

#[test]
fn projection_rendering_settings_uses_the_bounded_six_section_product_structure() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = populated_snapshot();
    let size = egui::vec2(1_440.0, 1_000.0);

    let (_initial, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    let (_opened, _full) =
        click_accessible_control_at(&ctx, &mut view, &snapshot, &full, "Settings", size);
    let (_settled, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, size);

    let update = full
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("Settings should expose AccessKit");
    let dialog = update
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            (node.role() == egui::accesskit::Role::Dialog && node.label() == Some("Settings"))
                .then_some(node)
        })
        .expect("Settings dialog should be present");
    let mut pending = dialog.children().to_vec();
    let mut dialog_bounds: Option<egui::accesskit::Rect> = None;
    while let Some(id) = pending.pop() {
        let node = update
            .nodes
            .iter()
            .find_map(|(candidate, node)| (*candidate == id).then_some(node))
            .expect("Settings dialog descendants should be present");
        if let Some(bounds) = node.bounds() {
            dialog_bounds = Some(match dialog_bounds {
                Some(accumulated) => egui::accesskit::Rect {
                    x0: accumulated.x0.min(bounds.x0),
                    y0: accumulated.y0.min(bounds.y0),
                    x1: accumulated.x1.max(bounds.x1),
                    y1: accumulated.y1.max(bounds.y1),
                },
                None => bounds,
            });
        }
        pending.extend(node.children().iter().copied());
    }
    let dialog_bounds = dialog_bounds.expect("Settings content should expose bounds");
    assert!(
        dialog_bounds.x1 - dialog_bounds.x0 <= 920.0,
        "Settings must remain within its 920px width bound: {dialog_bounds:?}"
    );
    assert!(
        dialog_bounds.y1 - dialog_bounds.y0 <= 720.0,
        "Settings must remain within its 720px height bound: {dialog_bounds:?}"
    );

    for section in [
        "Appearance",
        "Editor",
        "AI Providers",
        "Notifications",
        "Privacy",
        "Advanced",
    ] {
        assert!(
            accesskit_has_clickable_label(&full, section),
            "Settings must expose the {section} section"
        );
    }
    assert!(!accesskit_has_label(&full, "Models"));

    let (_advanced, _full) =
        click_accessible_control_at(&ctx, &mut view, &snapshot, &full, "Advanced", size);
    let (_settled, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    let (indexed, _) = click_accessible_control_at(
        &ctx,
        &mut view,
        &snapshot,
        &full,
        "Indexed workspace search",
        size,
    );
    assert_eq!(
        indexed.actions,
        vec![DesktopAction::SetIndexedWorkspaceSearchEnabled { enabled: true }]
    );
}

#[test]
fn projection_rendering_setup_is_one_four_item_checklist() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = populated_snapshot();

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (_opened, _full) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Setup");
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let setup_text = accesskit_dialog_text(&full, "Welcome to Legion");

    assert_eq!(
        setup_text
            .iter()
            .filter(|row| row.starts_with("Step "))
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4,
        "Setup should present exactly one four-item checklist: {setup_text:?}"
    );
    for item in [
        "Step 1 · Open and trust a workspace",
        "Step 2 · Optionally configure an AI provider",
        "Step 3 · Review privacy and reporting",
        "Step 4 · Learn Manual, Assist, Delegate, and Legion Workflows",
    ] {
        assert!(
            setup_text.iter().any(|row| row == item),
            "Setup checklist is missing `{item}`: {setup_text:?}"
        );
    }
    for old_section in ["Workspace", "Privacy and providers", "Keyboard and modes"] {
        assert!(
            !setup_text.iter().any(|row| row == old_section),
            "Setup must not split the checklist into the old `{old_section}` section"
        );
    }
}

#[test]
fn projection_rendering_provider_credentials_live_in_settings_ai_providers_section() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = assist_inline_prediction_snapshot();

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(!accesskit_has_label(&full, "AI Providers"));
    assert!(!accesskit_contains_text_in_x_range(
        &full,
        "Anthropic BYOK",
        1_115.0..=1_440.0
    ));

    let (_opened, _full) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Settings");
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(accesskit_has_label(&full, "AI Providers"));
    let (_providers, _full) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "AI Providers");
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(accesskit_has_label(
        &full,
        "Preferred AI provider: auto. Auto tries providers available on this computer before remote providers."
    ));
    assert!(accesskit_has_label(
        &full,
        "Anthropic API key — stored securely in the operating system keyring and never in workspace files."
    ));
    let settings_text = accesskit_dialog_text(&full, "Settings");
    for internal in ["BYOK", "loopback", "Preferred route"] {
        assert!(
            settings_text.iter().all(|row| !row.contains(internal)),
            "AI Providers should not expose `{internal}`: {settings_text:?}"
        );
    }
    let (provider, _) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Ollama");
    assert_eq!(
        provider.actions,
        vec![DesktopAction::SetPreferredAiProvider {
            provider_id: "ollama".to_string(),
        }]
    );
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (clear_key, _) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Clear Anthropic key");
    assert_eq!(
        clear_key.actions,
        vec![DesktopAction::DeleteProviderApiKey {
            provider_id: "anthropic".to_string(),
        }]
    );
}

#[test]
fn projection_rendering_empty_ai_providers_uses_plain_product_copy() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = Shell::empty("AI provider setup").projection_snapshot();

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (_opened, _full) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Settings");
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (_providers, _full) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "AI Providers");
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);

    assert!(accesskit_has_label(&full, "No AI provider configured"));
    assert!(accesskit_has_label(
        &full,
        "Choose an AI provider available on this computer or add an Anthropic API key."
    ));
    let settings_text = accesskit_dialog_text(&full, "Settings");
    for internal in [
        "model provider",
        "local route",
        "bring-your-own-key provider",
    ] {
        assert!(
            settings_text.iter().all(|row| !row.contains(internal)),
            "AI Providers must not expose `{internal}`: {settings_text:?}"
        );
    }
}

#[test]
fn projection_rendering_delegate_feedback_does_not_send_unentered_copy() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Delegate;

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(accesskit_has_label(&full, "Task intent"));
    assert!(!accesskit_has_label(&full, "Human Feedback"));
    assert!(!accesskit_has_clickable_label(&full, "Send"));
}

#[test]
fn projection_rendering_authority_ribbon_is_28px_below_top_bar() {
    let ctx = egui::Context::default();
    let mut view = ProjectionView::new();
    let snapshot = Shell::empty("Authority ribbon").projection_snapshot();

    let _ = render_projection_frame(&ctx, &mut view, &snapshot);
    let rects = view
        .last_shell_panel_rects()
        .expect("the composed shell must record panel rectangles");

    assert!((rects.authority.height() - 28.0).abs() <= 1.0);
    assert!((rects.authority.top() - rects.top.bottom()).abs() <= 1.0);

    for (label, top) in [
        ("left workspace surface", rects.left.top()),
        ("right workspace surface", rects.right.top()),
        ("center workspace surface", rects.center.top()),
    ] {
        assert!(
            (top - rects.top.bottom() - 28.0).abs() <= 1.0,
            "{label} must begin immediately below the 28px authority ribbon; top={top}, command_bar_bottom={}",
            rects.top.bottom()
        );
    }
}

#[test]
fn projection_rendering_authority_ribbon_uses_exact_mode_baselines() {
    for (mode, expected) in [
        (DockMode::Manual, "Manual · AI off · Workspace tools only"),
        (DockMode::Assist, "Assist · Suggestions require acceptance"),
        (
            DockMode::Delegate,
            "Delegate · Workspace scope · Changes remain proposals",
        ),
        (
            DockMode::Automate,
            "Workflows · Reviews remain approval-gated",
        ),
    ] {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut view = ProjectionView::new();
        let mut snapshot = Shell::empty("Authority baseline").projection_snapshot();
        snapshot.product_mode = mode;

        let (_frame, full) = render_projection_frame(&ctx, &mut view, &snapshot);
        assert!(
            accesskit_has_label(&full, expected),
            "{mode:?} must render its exact authority baseline `{expected}`"
        );
    }
}

#[test]
fn projection_rendering_authority_ribbon_surfaces_projected_readiness_and_boundary() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Assist;
    snapshot.assisted_ai_projection.providers = vec![AssistedAiProviderCapabilitySummary {
        provider_id: "local".to_string(),
        provider_label: "Local".to_string(),
        provider_class: AssistedAiProviderClass::Local,
        supported_operations: vec![AssistedAiOperationClass::ProposeEdit],
        supported_operation_count: 1,
        model_capability_label_count: 1,
        tool_capability_label_count: 0,
        context_window_label: "bounded".to_string(),
        cost_budget_label: "free".to_string(),
        risk_budget_label: "review required".to_string(),
        privacy_retention_label: "local only".to_string(),
        availability: AssistedAiProviderAvailabilityState::Available,
        refusal: None,
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];
    snapshot.assisted_ai_projection.provider_count = 1;
    snapshot.approval_checklist_projection.ready_for_approval = true;

    let (_frame, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    for expected in [
        "Workspace scope",
        "Provider ready",
        "Ready for approval · acceptance still required",
    ] {
        assert!(
            accesskit_has_label(&full, expected),
            "authority ribbon must surface projected context `{expected}`"
        );
    }
    for prohibited in ["Workspace 1", "1 provider ready"] {
        assert!(
            !accesskit_has_label(&full, prohibited),
            "authority ribbon must not expose raw projected detail `{prohibited}`"
        );
    }
}

#[test]
fn projection_rendering_authority_blocker_is_qualitative() {
    let mut snapshot = Shell::empty("Approval blocker").projection_snapshot();
    snapshot.approval_checklist_projection.blockers = vec![ApprovalChecklistReason {
        gate: ApprovalChecklistGateKind::PermissionBudget,
        reason_code: "approval.required".to_string(),
        label: "Approval required".to_string(),
        target_id: None,
        budget_id: None,
        capability: None,
        risk_label: ProposalRiskLabel::Medium,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    assert_eq!(
        model.authority_ribbon.approval_boundary.as_deref(),
        Some("Approval blocked")
    );
}

#[test]
fn projection_rendering_authority_pending_gates_are_qualitative() {
    let mut snapshot = Shell::empty("Approval gates").projection_snapshot();
    snapshot.approval_checklist_projection.gates = vec![ApprovalChecklistGateSummary {
        gate: ApprovalChecklistGateKind::AuditBeforeSuccess,
        status: ApprovalChecklistGateStatus::Unknown,
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        labels: Vec::new(),
        reasons: Vec::new(),
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];

    let model = DesktopProjectionViewModel::from_snapshot(&snapshot);
    assert_eq!(
        model.authority_ribbon.approval_boundary.as_deref(),
        Some("Approval gates remain")
    );
}

#[test]
fn projection_rendering_narrow_authority_ribbon_prioritizes_baseline_without_overflow() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Assist;
    snapshot.assisted_ai_projection.providers = vec![AssistedAiProviderCapabilitySummary {
        provider_id: "local".to_string(),
        provider_label: "Local".to_string(),
        provider_class: AssistedAiProviderClass::Local,
        supported_operations: vec![AssistedAiOperationClass::ProposeEdit],
        supported_operation_count: 1,
        model_capability_label_count: 1,
        tool_capability_label_count: 0,
        context_window_label: "bounded".to_string(),
        cost_budget_label: "free".to_string(),
        risk_budget_label: "review required".to_string(),
        privacy_retention_label: "local only".to_string(),
        availability: AssistedAiProviderAvailabilityState::Available,
        refusal: None,
        risk_label: ProposalRiskLabel::Low,
        privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
        redaction_hints: vec![RedactionHint::MetadataOnly],
        schema_version: 1,
    }];
    snapshot.approval_checklist_projection.ready_for_approval = true;
    let size = egui::vec2(480.0, 720.0);

    let (_frame, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    let rects = view
        .last_shell_panel_rects()
        .expect("the composed shell must record panel rectangles");
    let baseline = accesskit_bounds(&full, "Assist · Suggestions require acceptance", false);
    assert!(baseline.x0 >= f64::from(rects.authority.left()) - 1.0);
    assert!(baseline.x1 <= f64::from(rects.authority.right()) + 1.0);
    assert!(baseline.y0 >= f64::from(rects.authority.top()) - 1.0);
    assert!(baseline.y1 <= f64::from(rects.authority.bottom()) + 1.0);
    for optional in [
        "Workspace scope",
        "Provider ready",
        "Ready for approval · acceptance still required",
    ] {
        assert!(
            !accesskit_has_label(&full, optional),
            "narrow authority ribbon must hide optional detail `{optional}` before it can overflow"
        );
    }

    let update = full
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("authority ribbon should expose AccessKit");
    for (_id, node) in &update.nodes {
        let Some(bounds) = node.bounds() else {
            continue;
        };
        let inside_authority_band = bounds.y0 >= f64::from(rects.authority.top()) - 1.0
            && bounds.y1 <= f64::from(rects.authority.bottom()) + 1.0;
        if inside_authority_band && node.label().is_some() {
            assert!(
                bounds.x0 >= f64::from(rects.authority.left()) - 1.0
                    && bounds.x1 <= f64::from(rects.authority.right()) + 1.0,
                "authority label {:?} must not overflow the ribbon; bounds={bounds:?}, ribbon={:?}",
                node.label(),
                rects.authority
            );
        }
    }
}

#[test]
fn projection_rendering_each_mode_exposes_authority_status_in_shell_hierarchy() {
    for (mode, expected) in [
        (DockMode::Manual, "Manual · AI off · Workspace tools only"),
        (DockMode::Assist, "Assist · Suggestions require acceptance"),
        (
            DockMode::Delegate,
            "Delegate · Workspace scope · Changes remain proposals",
        ),
        (
            DockMode::Automate,
            "Workflows · Reviews remain approval-gated",
        ),
    ] {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut view = ProjectionView::new();
        let mut snapshot = Shell::empty("Authority hierarchy").projection_snapshot();
        snapshot.product_mode = mode;

        let (_frame, full) = render_projection_frame(&ctx, &mut view, &snapshot);
        let rects = view
            .last_shell_panel_rects()
            .expect("the composed shell must record panel rectangles");
        let node = full
            .platform_output
            .accesskit_update
            .as_ref()
            .expect("authority hierarchy should expose AccessKit")
            .nodes
            .iter()
            .find_map(|(_id, node)| {
                (node.label() == Some(expected) || node.value() == Some(expected)).then_some(node)
            })
            .unwrap_or_else(|| panic!("{mode:?} authority status must be rendered"));
        let bounds = node.bounds().expect("authority status must have bounds");

        assert_eq!(node.role(), egui::accesskit::Role::Status);
        assert!(bounds.y0 >= f64::from(rects.top.bottom()) - 1.0);
        assert!(bounds.y1 <= f64::from(rects.left.top()) + 1.0);
    }
}

#[test]
fn projection_rendering_primary_shell_controls_meet_28px_semantic_target() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = Shell::empty("Semantic targets").projection_snapshot();

    let (_frame, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    for label in [
        "Manual",
        "Assist",
        "Delegate",
        "Legion Workflows",
        "Command",
    ] {
        let bounds = accesskit_bounds(&full, label, true);
        assert!(
            bounds.y1 - bounds.y0 >= 28.0,
            "primary shell control `{label}` must use the semantic >=28px target; bounds={bounds:?}"
        );
    }
}

#[test]
fn projection_rendering_missing_assist_provider_uses_plain_product_copy() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = Shell::empty("Provider prerequisite").projection_snapshot();
    snapshot.product_mode = DockMode::Assist;

    let (_frame, full) = render_projection_frame(&ctx, &mut view, &snapshot);

    assert_eq!(
        accesskit_label_count(&full, "Choose an AI provider to enable predictions."),
        1
    );
    assert!(accesskit_has_clickable_label(&full, "Settings"));
    assert!(!accesskit_has_label(&full, "Assist workbench"));
    assert!(!accesskit_has_label(&full, "Inline prediction"));
    assert!(!accesskit_has_label(&full, "Context Chips"));
    assert!(!accesskit_has_label(&full, "Model Picker"));
    assert!(!accesskit_has_label(&full, "Anthropic BYOK"));

    let (opened, _full) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Settings");
    assert_eq!(opened.actions, vec![DesktopAction::OpenSettings]);
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(accesskit_has_label(&full, "AI Providers"));
}

#[test]
fn projection_rendering_shell_panels_preserve_physical_prototype_edges() {
    let assert_edge = |label: &str, actual: f32, expected: f32| {
        assert!(
            (actual - expected).abs() <= 2.0,
            "{label}: panel edge {actual} must align with {expected} within the 2px separator stroke"
        );
    };
    for size in [egui::vec2(960.0, 720.0), egui::vec2(1_440.0, 900.0)] {
        let ctx = egui::Context::default();
        let mut view = ProjectionView::new();
        let snapshot = Shell::empty("Panel geometry").projection_snapshot();

        let _ = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
        let rects = view
            .last_shell_panel_rects()
            .expect("the composed shell must record panel rectangles");
        let geometry = ShellGeometry::for_available_size(size.x, size.y);
        let dock_bottom = if geometry.compact {
            rects.status.top() - 28.0
        } else {
            rects.status.top()
        };

        assert_edge("top left", rects.top.left(), 0.0);
        assert_edge("top right", rects.top.right(), size.x);
        assert_edge("status left", rects.status.left(), 0.0);
        assert_edge("status right", rects.status.right(), size.x);
        assert_edge("authority top", rects.authority.top(), rects.top.bottom());
        assert_edge("authority left", rects.authority.left(), 0.0);
        assert_edge("authority right", rects.authority.right(), size.x);
        assert_edge("left top", rects.left.top(), rects.authority.bottom());
        assert_edge("left bottom", rects.left.bottom(), dock_bottom);
        assert_edge("right top", rects.right.top(), rects.authority.bottom());
        assert_edge("right bottom", rects.right.bottom(), dock_bottom);
        assert_edge("bottom left", rects.bottom.left(), rects.left.right());
        assert_edge("bottom right", rects.bottom.right(), rects.right.left());
        assert_edge("bottom bottom", rects.bottom.bottom(), dock_bottom);
        assert_edge("center left", rects.center.left(), rects.left.right());
        assert_edge("center right", rects.center.right(), rects.right.left());
        assert_edge("center top", rects.center.top(), rects.authority.bottom());
        assert_edge("center bottom", rects.center.bottom(), rects.bottom.top());
        assert_edge(
            "console height",
            rects.bottom.height(),
            geometry.bottom_height,
        );
    }
}

#[test]
fn projection_rendering_contextual_modes_share_inspector_width_while_manual_reclaims_it() {
    let ctx = egui::Context::default();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.active_buffer_projection.workspace_id = Some(WorkspaceId(u128::MAX));
    snapshot.active_buffer_projection.file_path = Some(CanonicalPath(
        r"D:\legion-ide\.worktrees\ui-prototype-polish\crates\legion-desktop\src\view.rs"
            .to_string(),
    ));
    let size = egui::vec2(1_440.0, 900.0);
    let expected = ShellGeometry::for_available_size(size.x, size.y).right_width;
    let mut observed = Vec::new();

    for mode in [
        DockMode::Manual,
        DockMode::Assist,
        DockMode::Delegate,
        DockMode::Automate,
    ] {
        snapshot.product_mode = mode;
        for _ in 0..3 {
            let _ = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
        }
        let rects = view
            .last_shell_panel_rects()
            .expect("the composed shell must record panel rectangles");
        let editor = view
            .last_editor_rect()
            .expect("the real editor surface should record its allocation");
        observed.push((mode, rects.right.width(), editor.width()));
    }

    let manual = observed[0];
    assert!(
        manual.1 <= 2.0,
        "Manual must not reserve an inspector: {observed:?}"
    );
    for (mode, right_width, _editor_width) in &observed[1..] {
        assert!(
            (right_width - expected).abs() <= 2.0,
            "{mode:?} content must not expand the default {expected}px right rail; observed={observed:?}"
        );
    }
    let contextual_editor_width = observed[1].2;
    for (mode, _right_width, editor_width) in &observed[2..] {
        assert!(
            (editor_width - contextual_editor_width).abs() <= 2.0,
            "{mode:?} content must preserve the contextual inspector allocation; observed={observed:?}"
        );
    }
    assert!(
        manual.2 > contextual_editor_width,
        "Manual should reclaim inspector width for the editor: {observed:?}"
    );
}

#[test]
fn projection_rendering_context_pills_wrap_as_atomic_readable_items() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Assist;
    snapshot.active_buffer_projection.workspace_id = Some(WorkspaceId(u128::MAX));
    snapshot.active_buffer_projection.file_path = Some(CanonicalPath(
        r"D:\legion-ide\.worktrees\ui-prototype-polish\crates\legion-desktop\src\view.rs"
            .to_string(),
    ));

    let (_frame, full) =
        render_projection_frame_at(&ctx, &mut view, &snapshot, egui::vec2(1_440.0, 900.0));
    let manifest = accesskit_bounds(&full, "manifest: 1 items", false);
    assert!(
        manifest.x1 - manifest.x0 >= 80.0 && manifest.y1 - manifest.y0 <= 30.0,
        "the manifest context pill must remain a horizontal atomic item; bounds={manifest:?}"
    );
    let manifest_node = full
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("AccessKit update should be enabled")
        .nodes
        .iter()
        .find_map(|(_id, node)| (node.label() == Some("manifest: 1 items")).then_some(node))
        .expect("the manifest context pill should expose one semantic node");
    assert_eq!(manifest_node.role(), egui::accesskit::Role::Label);
    assert!(!manifest_node.supports_action(egui::accesskit::Action::Click));
    assert!(!manifest_node.supports_action(egui::accesskit::Action::Focus));
    assert!(accesskit_has_label(&full, "workspace: current"));
    assert!(!accesskit_contains_text_in_x_range(
        &full,
        &u128::MAX.to_string(),
        1_115.0..=1_440.0
    ));
    assert!(!accesskit_contains_text_in_x_range(
        &full,
        "projected",
        1_115.0..=1_440.0
    ));
}

#[test]
fn projection_rendering_desktop_top_bar_uses_three_non_overlapping_regions() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = Shell::empty("Legion IDE").projection_snapshot();

    let (_frame, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let manual = accesskit_button_bounds_in_y_range(&full, "Manual", 0.0..=44.0);
    let workflows = accesskit_button_bounds_in_y_range(&full, "Legion Workflows", 0.0..=44.0);
    let command = accesskit_button_bounds_in_y_range(&full, "Command", 0.0..=44.0);
    let workspace = accesskit_bounds(&full, "Legion IDE", false);
    let switch_center = ((manual.x0 + workflows.x1) * 0.5) as f32;

    assert!(
        (switch_center - 720.0).abs() <= 24.0,
        "desktop mode switch must be centered around x=720; actual center={switch_center}, manual={manual:?}, workflows={workflows:?}, command={command:?}, workspace={workspace:?}"
    );
    assert!(
        command.x0 >= 1_160.0,
        "Command must be right-aligned within the 280px edge region; bounds={command:?}"
    );
    assert!(workspace.x1 <= manual.x0);
    assert!(workflows.x1 <= command.x0);
    assert!(accesskit_has_label(&full, "·"));
}

#[test]
fn projection_rendering_compact_assist_inspector_keeps_status_reachable() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Assist;
    let (_initial, full) =
        render_projection_frame_at(&ctx, &mut view, &snapshot, egui::vec2(960.0, 720.0));
    let (_drawer, _full) = click_accessible_control_at(
        &ctx,
        &mut view,
        &snapshot,
        &full,
        "Inspector drawer",
        egui::vec2(960.0, 720.0),
    );
    let (_settled, full) =
        render_projection_frame_at(&ctx, &mut view, &snapshot, egui::vec2(960.0, 720.0));
    assert!(accesskit_has_label(&full, "Inline prediction"));
    assert!(accesskit_has_label(&full, "Context"));
    assert!(!accesskit_has_label(&full, "Assist workbench"));
}

#[test]
fn projection_rendering_empty_workflows_use_plain_product_copy_without_invented_state() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = Shell::empty("Empty workflow").projection_snapshot();
    snapshot.product_mode = DockMode::Automate;

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(accesskit_has_label(&full, "Legion Workflows"));
    assert!(accesskit_has_label(&full, "No workflow sessions yet"));
    assert!(accesskit_has_label(
        &full,
        "Start a workflow to see its progress here."
    ));
    assert!(!accesskit_has_label(
        &full,
        "No workflow sessions projected"
    ));
    assert!(!accesskit_has_label(&full, "Running"));
    assert!(!accesskit_has_label(&full, "96k / 250k"));
    assert_eq!(accesskit_label_count(&full, "No workflow sessions yet"), 1);
    for absent in [
        "Legion Workflows workbench",
        "Force Review",
        "Pause Workflow",
        "Add Constraint",
        "Resource budgets",
        "Risk gate",
        "No budget rows projected",
        "No risk gate projected",
    ] {
        assert!(
            !accesskit_has_label(&full, absent),
            "empty Workflows must not expose `{absent}`"
        );
    }
}

#[test]
fn projection_rendering_editor_tabs_expose_tab_state_and_named_close_buttons() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = populated_snapshot();

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let update = full
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("editor tabs should expose AccessKit");
    for (label, selected) in [("Cargo.toml", true), ("lib.rs", false)] {
        let node = update
            .nodes
            .iter()
            .find_map(|(_id, node)| {
                (node.label() == Some(label) && node.role() == egui::accesskit::Role::Tab)
                    .then_some(node)
            })
            .unwrap_or_else(|| panic!("editor tab `{label}` should expose Role::Tab"));
        assert_eq!(node.is_selected(), Some(selected));
        assert_eq!(
            node.aria_current(),
            selected.then_some(egui::accesskit::AriaCurrent::True)
        );
        assert!(node.supports_action(egui::accesskit::Action::Click));
    }
    for label in ["Close Cargo.toml", "Close lib.rs"] {
        let close = update
            .nodes
            .iter()
            .find_map(|(_id, node)| {
                (node.label() == Some(label)
                    && node.role() == egui::accesskit::Role::Button
                    && node.supports_action(egui::accesskit::Action::Click))
                .then_some(node)
            })
            .unwrap_or_else(|| panic!("named tab close control `{label}` should be exposed"));
        let bounds = close
            .bounds()
            .expect("tab close control should have bounds");
        assert!(bounds.x1 - bounds.x0 >= 24.0);
        assert!(bounds.y1 - bounds.y0 >= 24.0);
    }
}

#[test]
fn projection_rendering_manual_empty_terminal_never_falls_back_to_agent_diagnostics() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = Shell::empty("Manual terminal").projection_snapshot();

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(accesskit_has_label(&full, "No terminal activity"));
    let update = full
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("manual terminal should expose AccessKit");
    for forbidden in ["workflow activity:", "agent stream:"] {
        assert!(
            update.nodes.iter().all(|(_id, node)| {
                !node.label().is_some_and(|label| label.contains(forbidden))
            }),
            "Manual terminal fallback must not render `{forbidden}` diagnostics"
        );
    }
}

#[test]
fn projection_rendering_active_terminal_controls_meet_shared_minimum_target() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = Shell::empty("Active terminal").projection_snapshot();
    snapshot.terminal_panel_projection.active_session_id = Some(TerminalSessionId(7));

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    for label in ["Send", "Poll", "Kill", "Close"] {
        let bounds = accesskit_bounds(&full, label, true);
        assert!(
            bounds.x1 - bounds.x0 >= 24.0 && bounds.y1 - bounds.y0 >= 24.0,
            "terminal action `{label}` must use the shared >=24px target; bounds={bounds:?}"
        );
    }
}

#[test]
fn projection_rendering_bottom_tabs_are_state_authoritative_and_activity_is_mode_independent() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Delegate;
    let mut state = DesktopProjectionViewState::default();

    let (initial, mut full) =
        render_projection_frame_with_state(&ctx, &mut view, &snapshot, &state);
    assert_eq!(initial.selected_bottom_panel, BottomPanelTab::Terminal);
    let terminal = accesskit_bounds(&full, "TERMINAL", true);
    assert!(terminal.y1 - terminal.y0 >= 24.0);
    assert!(accesskit_has_label(&full, "Terminal / Runtime"));

    let (problems_click_frame, next) = click_accessible_control_with_state(
        &ctx,
        &mut view,
        &snapshot,
        &state,
        &full,
        "PROBLEMS (0)",
    );
    full = next;
    assert_eq!(
        problems_click_frame.selected_bottom_panel,
        BottomPanelTab::Problems
    );
    state.selected_bottom_panel = problems_click_frame.selected_bottom_panel;
    assert!(accesskit_has_label(&full, "Problems"));
    assert!(!accesskit_has_label(&full, "Terminal / Runtime"));
    assert!(
        problems_click_frame
            .bottom_tab_rows
            .iter()
            .any(|row| { row.contains("id=problems") && row.contains("active=true") })
    );
    assert!(
        problems_click_frame
            .bottom_tab_rows
            .iter()
            .any(|row| { row.contains("id=term") && row.contains("active=false") })
    );

    snapshot.product_mode = DockMode::Assist;
    let (assist, next) = render_projection_frame_with_state(&ctx, &mut view, &snapshot, &state);
    full = next;
    assert_eq!(
        assist.selected_bottom_panel,
        BottomPanelTab::Problems,
        "valid bottom-panel selection should survive mode changes"
    );

    let (agent_click_frame, next) =
        click_accessible_control_with_state(&ctx, &mut view, &snapshot, &state, &full, "ACTIVITY");
    full = next;
    assert_eq!(
        agent_click_frame.selected_bottom_panel,
        BottomPanelTab::Activity
    );
    state.selected_bottom_panel = agent_click_frame.selected_bottom_panel;
    assert!(accesskit_has_label(&full, "Activity"));
    assert!(
        agent_click_frame
            .bottom_tab_rows
            .iter()
            .any(|row| { row.contains("id=activity") && row.contains("active=true") })
    );
    assert!(
        agent_click_frame
            .bottom_tab_rows
            .iter()
            .any(|row| { row.contains("id=term") && row.contains("active=false") })
    );

    snapshot.product_mode = DockMode::Manual;
    let (manual_frame, full) =
        render_projection_frame_with_state(&ctx, &mut view, &snapshot, &state);
    assert_eq!(
        manual_frame.selected_bottom_panel,
        BottomPanelTab::Activity,
        "Activity selection must remain app-authoritative across mode changes"
    );
    assert!(accesskit_has_label(&full, "ACTIVITY"));
    assert!(accesskit_has_label(&full, "Activity"));
    assert!(!accesskit_has_label(&full, "Terminal / Runtime"));
    assert!(
        manual_frame
            .bottom_tab_rows
            .iter()
            .any(|row| { row.contains("id=activity") && row.contains("active=true") })
    );
    assert!(
        manual_frame
            .bottom_tab_rows
            .iter()
            .any(|row| row.contains("id=diagnostics"))
    );

    snapshot.product_mode = DockMode::Assist;
    let (restored, full) = render_projection_frame_with_state(&ctx, &mut view, &snapshot, &state);
    assert_eq!(restored.selected_bottom_panel, BottomPanelTab::Activity);
    assert!(accesskit_has_label(&full, "Activity"));
}

#[test]
fn projection_rendering_expanded_workbenches_leave_a_usable_visible_editor() {
    for size in [egui::vec2(960.0, 720.0), egui::vec2(1_440.0, 900.0)] {
        for (mode, disclosure, action) in [(DockMode::Delegate, "Delegate workbench", "Approve")] {
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
            assert!(
                visible_editor.width() >= 360.0,
                "{mode:?} at {size:?} must leave at least 360px of the editor visible; actual editor={editor_rect:?}, visible={visible_editor:?}"
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
    assert!(!accesskit_has_label(&full, "Assist workbench"));
    let (assist_action, _) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Predict");
    assert!(matches!(
        assist_action.actions.as_slice(),
        [DesktopAction::RequestAssistInlinePrediction { .. }]
    ));

    snapshot.product_mode = DockMode::Delegate;
    let (_delegate, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let header = accesskit_bounds(&full, "Delegate workbench", true);
    let editor = accesskit_bounds(&full, "[workspace]", false);
    assert!(
        header.y1 <= editor.y0,
        "the active Delegate disclosure must be allocated before the editor scroll surface"
    );
    let (_opened, _) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Delegate workbench");
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
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
    assert!(!accesskit_has_label(&full, "Legion Workflows workbench"));
    assert!(!accesskit_has_label(&full, "Force Review"));
}

#[test]
fn projection_rendering_explorer_does_not_render_the_legacy_workbench_toolbox() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Assist;

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(!accesskit_has_label(&full, "Workbench tools"));
    assert!(accesskit_has_label(&full, "EXPLORER ·"));
    assert!(!accesskit_has_label(&full, "Refresh Git"));
    assert!(!accesskit_has_label(&full, "Refresh tests"));
    assert!(!accesskit_has_label(&full, "Refresh configs"));
}

#[test]
fn projection_rendering_activity_surfaces_keep_explorer_workspace_only_and_actions_wired() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Assist;

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(accesskit_has_label(&full, "EXPLORER ·"));
    assert!(
        !accesskit_has_label(&full, "Workbench tools"),
        "Explorer must contain workspace navigation rather than a mixed tool drawer"
    );
    for label in [
        "Explorer",
        "Search",
        "Symbols",
        "Source Control",
        "Tests",
        "Run and Debug",
    ] {
        let bounds = accesskit_button_bounds_in_x_range(&full, label, 0.0..=46.0);
        assert!(bounds.x1 - bounds.x0 <= 38.0, "{label}: {bounds:?}");
        assert!(bounds.y1 - bounds.y0 >= 24.0, "{label}: {bounds:?}");
    }

    let (source_control, full) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Source Control");
    assert!(source_control.actions.is_empty());
    assert!(accesskit_has_label(&full, "SOURCE CONTROL"));
    assert!(!accesskit_has_label(&full, "EXPLORER ·"));
    let (git, full) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Refresh Git");
    assert_eq!(git.actions, vec![DesktopAction::RefreshGit]);

    let (_tests_surface, full) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Tests");
    assert!(accesskit_has_label(&full, "TESTS"));
    let (tests, full) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Refresh tests");
    assert_eq!(tests.actions, vec![DesktopAction::RefreshTestExplorer]);

    let (_debug_surface, full) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Run and Debug");
    assert!(accesskit_has_label(&full, "RUN AND DEBUG"));
    let (debug, full) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Refresh configs");
    assert_eq!(
        debug.actions,
        vec![DesktopAction::RefreshDebugConfigurations]
    );

    let search_bounds = accesskit_button_bounds_in_x_range(&full, "Search", 0.0..=46.0);
    let (search, full) = click_projection_at(
        &ctx,
        &mut view,
        &snapshot,
        egui::pos2(
            ((search_bounds.x0 + search_bounds.x1) * 0.5) as f32,
            ((search_bounds.y0 + search_bounds.y1) * 0.5) as f32,
        ),
        egui::vec2(1_440.0, 900.0),
    );
    assert_eq!(
        search.actions,
        vec![DesktopAction::OpenPalette {
            mode: PaletteMode::Search,
            query: "/".to_string(),
            scope: SearchScopeProjection::ActiveFile,
        }]
    );
    assert!(accesskit_has_label(&full, "SEARCH"));

    let symbols_bounds = accesskit_button_bounds_in_x_range(&full, "Symbols", 0.0..=46.0);
    let (symbols, full) = click_projection_at(
        &ctx,
        &mut view,
        &snapshot,
        egui::pos2(
            ((symbols_bounds.x0 + symbols_bounds.x1) * 0.5) as f32,
            ((symbols_bounds.y0 + symbols_bounds.y1) * 0.5) as f32,
        ),
        egui::vec2(1_440.0, 900.0),
    );
    assert_eq!(
        symbols.actions,
        vec![DesktopAction::OpenPalette {
            mode: PaletteMode::Symbol,
            query: String::new(),
            scope: SearchScopeProjection::ActiveFile,
        }]
    );
    assert!(accesskit_has_label(&full, "SYMBOLS"));
}

#[test]
fn projection_rendering_activity_copy_and_raw_diagnostics_are_separate_destinations() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Assist;

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (activity, full) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "ACTIVITY");
    assert!(
        activity
            .bottom_tab_rows
            .iter()
            .any(|row| row.contains("id=activity") && row.contains("active=true"))
    );
    assert!(accesskit_has_label(&full, "Activity"));
    assert!(accesskit_has_label(
        &full,
        "1 assistant request in this workspace"
    ));
    assert!(accesskit_has_label(&full, "1 delegated plan available"));
    for raw in ["bottom console:", "workflow activity:", "agent stream:"] {
        assert!(
            !accesskit_contains_text_in_x_range(&full, raw, 294.0..=1_115.0),
            "user-facing Activity must not expose raw internal row `{raw}`"
        );
    }

    let diagnostics_bounds = accesskit_button_bounds_in_x_range(&full, "Diagnostics", 0.0..=46.0);
    let (_diagnostics, full) = click_projection_at(
        &ctx,
        &mut view,
        &snapshot,
        egui::pos2(
            ((diagnostics_bounds.x0 + diagnostics_bounds.x1) * 0.5) as f32,
            ((diagnostics_bounds.y0 + diagnostics_bounds.y1) * 0.5) as f32,
        ),
        egui::vec2(1_440.0, 900.0),
    );
    assert!(accesskit_has_label(&full, "Diagnostics"));
    assert!(accesskit_contains_text_in_x_range(
        &full,
        "workflow activity:",
        294.0..=1_115.0
    ));
    assert!(accesskit_contains_text_in_x_range(
        &full,
        "agent stream:",
        294.0..=1_115.0
    ));

    let (restored, full) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "ACTIVITY");
    assert!(
        restored
            .bottom_tab_rows
            .iter()
            .any(|row| row.contains("id=activity") && row.contains("active=true"))
    );
    assert!(accesskit_has_label(
        &full,
        "1 assistant request in this workspace"
    ));
    assert!(!accesskit_contains_text_in_x_range(
        &full,
        "workflow activity:",
        294.0..=1_115.0
    ));

    snapshot.product_mode = DockMode::Manual;
    let (_manual, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(accesskit_has_label(&full, "ACTIVITY"));
}

#[test]
fn projection_rendering_does_not_duplicate_palette_search_results_in_fixed_panels() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.search_projection = SearchProjection {
        query_id: Some("search:needle".to_string()),
        scope: SearchScopeProjection::Workspace,
        query_label: "needle".to_string(),
        status: SearchStatusProjection {
            kind: SearchStatusKindProjection::Completed,
            message: "Search completed".to_string(),
        },
        results: vec![SearchResultProjection {
            query_id: "search:needle".to_string(),
            scope: SearchScopeProjection::Workspace,
            workspace_id: None,
            buffer_id: None,
            file_id: None,
            file_path: Some(CanonicalPath("src/search-only.txt".to_string())),
            line_number: 0,
            range: ProtocolTextRange {
                start: TextCoordinate {
                    line: 0,
                    character: 0,
                    byte_offset: Some(0),
                    utf16_offset: Some(0),
                },
                end: TextCoordinate {
                    line: 0,
                    character: 6,
                    byte_offset: Some(6),
                    utf16_offset: Some(6),
                },
            },
            snippet: "needle appears only in the palette".to_string(),
            snippet_truncated: false,
            stale: false,
        }],
        result_limit: 20,
        omitted_result_count: 0,
        omitted_file_count: 0,
        skipped_binary_count: 0,
        case_sensitive: false,
        whole_word: false,
        use_regex: false,
        diagnostics: Vec::new(),
        generated_at: TimestampMillis(1),
        schema_version: 1,
    };

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let search_bounds = accesskit_button_bounds_in_x_range(&full, "Search", 0.0..=46.0);
    let (_selected, _full) = click_projection_at(
        &ctx,
        &mut view,
        &snapshot,
        egui::pos2(
            ((search_bounds.x0 + search_bounds.x1) * 0.5) as f32,
            ((search_bounds.y0 + search_bounds.y1) * 0.5) as f32,
        ),
        egui::vec2(1_440.0, 900.0),
    );
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(
        !accesskit_contains_text_in_x_range(
            &full,
            "needle appears only in the palette",
            0.0..=1_440.0,
        ),
        "fixed editor/sidebar panels must not duplicate palette results"
    );
    assert!(
        !accesskit_has_label(&full, "Search finished."),
        "search status belongs in the palette shell"
    );
}

#[test]
fn projection_rendering_settings_and_setup_are_mode_independent_utility_overlays() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Assist;

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    for label in ["Settings", "Setup"] {
        let bounds = accesskit_button_bounds_in_x_range(&full, label, 0.0..=46.0);
        assert!(bounds.x1 - bounds.x0 <= 38.0, "{label}: {bounds:?}");
        assert!(bounds.y1 - bounds.y0 >= 24.0, "{label}: {bounds:?}");
    }
    for forbidden in ["Settings", "First-run onboarding"] {
        assert!(
            !accesskit_contains_text_in_x_range(&full, forbidden, 1_115.0..=1_440.0),
            "contextual mode rails must not own `{forbidden}`"
        );
    }

    let settings_bounds = accesskit_button_bounds_in_x_range(&full, "Settings", 0.0..=46.0);
    let (opened, _full) = click_projection_at(
        &ctx,
        &mut view,
        &snapshot,
        egui::pos2(
            ((settings_bounds.x0 + settings_bounds.x1) * 0.5) as f32,
            ((settings_bounds.y0 + settings_bounds.y1) * 0.5) as f32,
        ),
        egui::vec2(1_440.0, 900.0),
    );
    assert_eq!(opened.actions, vec![DesktopAction::OpenSettings]);
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(accesskit_has_role(&full, egui::accesskit::Role::Dialog));
    for section in ["Appearance", "Editor", "Notifications", "Privacy"] {
        assert!(accesskit_has_clickable_label(&full, section));
    }
    assert!(accesskit_has_label(&full, "Theme"));
    assert!(!accesskit_has_label(&full, "Line numbers"));

    let (_editor, _full) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Editor");
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let settings_labels = full
        .platform_output
        .accesskit_update
        .as_ref()
        .map(|update| {
            update
                .nodes
                .iter()
                .filter_map(|(_id, node)| node.label().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    assert!(
        accesskit_has_label(&full, "Line numbers"),
        "Editor section labels: {settings_labels:?}"
    );
    assert!(!accesskit_has_label(&full, "Theme"));

    let (_closed, _full) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Close Settings");
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(!accesskit_has_role(&full, egui::accesskit::Role::Dialog));
    let setup_bounds = accesskit_button_bounds_in_x_range(&full, "Setup", 0.0..=46.0);
    let (_setup, _full) = click_projection_at(
        &ctx,
        &mut view,
        &snapshot,
        egui::pos2(
            ((setup_bounds.x0 + setup_bounds.x1) * 0.5) as f32,
            ((setup_bounds.y0 + setup_bounds.y1) * 0.5) as f32,
        ),
        egui::vec2(1_440.0, 900.0),
    );
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(accesskit_has_role(&full, egui::accesskit::Role::Dialog));
    assert!(accesskit_has_label(&full, "Welcome to Legion"));
    let (finished, _full) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Finish setup");
    assert_eq!(finished.actions, vec![DesktopAction::DismissOnboarding]);
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    assert!(!accesskit_has_role(&full, egui::accesskit::Role::Dialog));

    let state = DesktopProjectionViewState {
        first_run_onboarding_visible: true,
        ..DesktopProjectionViewState::default()
    };
    let mut first_run_view = ProjectionView::new();
    let (_first_run, full) =
        render_projection_frame_with_state(&ctx, &mut first_run_view, &snapshot, &state);
    assert!(!accesskit_has_role(&full, egui::accesskit::Role::Dialog));
    assert!(accesskit_has_clickable_label(&full, "Setup"));
    let (_opened, full) = click_accessible_control_with_state(
        &ctx,
        &mut first_run_view,
        &snapshot,
        &state,
        &full,
        "Setup",
    );
    assert!(accesskit_has_role(&full, egui::accesskit::Role::Dialog));
    assert!(accesskit_has_label(&full, "Welcome to Legion"));
}

#[test]
fn projection_rendering_escape_closes_utility_overlay_and_restores_trigger_focus() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = Shell::empty("Utility focus").projection_snapshot();
    let size = egui::vec2(1_440.0, 900.0);

    let (_initial, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    let (settings_node, settings_bounds) = full
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("utility rail should expose AccessKit")
        .nodes
        .iter()
        .find_map(|(id, node)| {
            let bounds = node.bounds()?;
            (node.label() == Some("Settings")
                && node.supports_action(egui::accesskit::Action::Focus)
                && bounds.x1 <= 46.0)
                .then_some((*id, bounds))
        })
        .expect("Settings utility should be keyboard focusable");
    let focus_input = desktop_raw_input_at(
        size,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Focus,
                target_tree: egui::accesskit::TreeId::ROOT,
                target_node: settings_node,
                data: None,
            },
        )],
    );
    let _ = ctx.run_ui(focus_input, |ui| {
        let _ = view.render(ui, &snapshot);
    });
    let trigger_focus = ctx
        .memory(|memory| memory.focused())
        .expect("AccessKit focus should reach Settings before it opens");

    let (_opened, _full) = click_projection_at(
        &ctx,
        &mut view,
        &snapshot,
        egui::pos2(
            ((settings_bounds.x0 + settings_bounds.x1) * 0.5) as f32,
            ((settings_bounds.y0 + settings_bounds.y1) * 0.5) as f32,
        ),
        size,
    );
    let (_settled, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    assert!(accesskit_has_role(&full, egui::accesskit::Role::Dialog));
    assert_ne!(ctx.memory(|memory| memory.focused()), Some(trigger_focus));

    let escape_input = desktop_raw_input_at(
        size,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: Some(egui::Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
    );
    let _ = ctx.run_ui(escape_input, |ui| {
        let _ = view.render(ui, &snapshot);
    });
    let (_restored, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    assert!(!accesskit_has_role(&full, egui::accesskit::Role::Dialog));
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        Some(trigger_focus),
        "Escape must restore focus to the utility control that opened the overlay"
    );
}

#[test]
fn projection_rendering_manual_reclaims_inspector_and_standard_inspector_resizes_within_bounds() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    let size = egui::vec2(1_440.0, 900.0);

    snapshot.product_mode = DockMode::Manual;
    let (_manual, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    let rects = view
        .last_shell_panel_rects()
        .expect("Manual shell should record its physical regions");
    assert!(
        rects.right.width() <= 2.0,
        "Manual must not reserve a right inspector solely to announce disabled AI: {rects:?}"
    );
    assert!(!accesskit_has_label(&full, "AI engine disengaged"));
    assert!(!accesskit_has_label(
        &full,
        "Product AI dispatch is disabled in Manual"
    ));
    assert!(accesskit_has_clickable_label(&full, "Assist"));
    let editor = view
        .last_editor_rect()
        .expect("Manual should retain the real editor")
        .intersect(egui::Rect::from_min_size(egui::Pos2::ZERO, size));
    assert!(editor.width() >= 560.0, "Manual editor width: {editor:?}");
    assert!(editor.height() >= 240.0, "Manual editor height: {editor:?}");

    snapshot.product_mode = DockMode::Assist;
    let inspector_panel_id = egui::Id::new("legion_desktop_trust");
    let _ = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    ctx.data_mut(|data| {
        data.insert_persisted(
            inspector_panel_id,
            egui::PanelState {
                rect: egui::Rect::from_min_size(
                    egui::pos2(size.x - 470.0, 70.0),
                    egui::vec2(470.0, 600.0),
                ),
            },
        );
    });
    let _ = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    let wide = view
        .last_shell_panel_rects()
        .expect("Assist shell should record inspector allocation")
        .right
        .width();
    assert!(
        (460.0..=480.0).contains(&wide),
        "standard inspector should accept a persisted width up to 480px; actual={wide}"
    );

    ctx.data_mut(|data| {
        data.insert_persisted(
            inspector_panel_id,
            egui::PanelState {
                rect: egui::Rect::from_min_size(
                    egui::pos2(size.x - 260.0, 70.0),
                    egui::vec2(260.0, 600.0),
                ),
            },
        );
    });
    let _ = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    let narrow = view
        .last_shell_panel_rects()
        .expect("Assist shell should retain inspector allocation")
        .right
        .width();
    assert!(
        (288.0..=300.0).contains(&narrow),
        "standard inspector must clamp to a 288px minimum; actual={narrow}"
    );
}

#[test]
fn projection_rendering_compact_shell_collapses_navigation_and_exposes_inspector_drawers() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Assist;
    let compact_size = egui::vec2(960.0, 720.0);

    let (_compact, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, compact_size);
    let compact_rects = view
        .last_shell_panel_rects()
        .expect("compact shell should record collapsed regions");
    assert!(
        compact_rects.left.width() <= 2.0,
        "Explorer must collapse before the editor at compact width: {compact_rects:?}"
    );
    assert!(
        compact_rects.right.width() <= 2.0,
        "Inspector must become a drawer at compact width: {compact_rects:?}"
    );
    let editor = view
        .last_editor_rect()
        .expect("compact shell should keep the real editor")
        .intersect(egui::Rect::from_min_size(egui::Pos2::ZERO, compact_size));
    assert!(editor.width() >= 360.0, "compact editor width: {editor:?}");
    assert!(
        editor.height() >= 180.0,
        "compact editor height: {editor:?}"
    );
    for label in ["Explorer drawer", "Inspector drawer", "Bottom panel drawer"] {
        let bounds = accesskit_bounds(&full, label, true);
        assert!(bounds.x1 - bounds.x0 >= 24.0, "{label}: {bounds:?}");
        assert!(bounds.y1 - bounds.y0 >= 24.0, "{label}: {bounds:?}");
    }

    let (_explorer, _full) = click_accessible_control_at(
        &ctx,
        &mut view,
        &snapshot,
        &full,
        "Explorer drawer",
        compact_size,
    );
    let (_settled, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, compact_size);
    assert!(accesskit_has_label(&full, "EXPLORER ·"));
    assert!(accesskit_has_clickable_label(&full, "Source Control"));
    assert!(accesskit_has_clickable_label(&full, "Settings"));

    let inspector_ctx = egui::Context::default();
    inspector_ctx.enable_accesskit();
    let mut inspector_view = ProjectionView::new();
    let (_compact, full) =
        render_projection_frame_at(&inspector_ctx, &mut inspector_view, &snapshot, compact_size);
    let (_inspector, _full) = click_accessible_control_at(
        &inspector_ctx,
        &mut inspector_view,
        &snapshot,
        &full,
        "Inspector drawer",
        compact_size,
    );
    let (_settled, full) =
        render_projection_frame_at(&inspector_ctx, &mut inspector_view, &snapshot, compact_size);
    assert!(accesskit_has_label(&full, "Context"));

    let standard_size = egui::vec2(1_184.0, 530.0);
    let standard_ctx = egui::Context::default();
    let mut standard_view = ProjectionView::new();
    let _ = render_projection_frame_at(&standard_ctx, &mut standard_view, &snapshot, standard_size);
    let standard = standard_view
        .last_shell_panel_rects()
        .expect("standard shell should retain docked regions");
    assert!(
        standard.center.width() >= 560.0,
        "standard editor region width: {standard:?}"
    );
    assert!(
        standard.center.height() >= 240.0,
        "standard editor region height: {standard:?}"
    );
}

#[test]
fn projection_rendering_symbols_setup_and_settings_use_plain_copy_while_diagnostics_keeps_raw_rows()
{
    let mut snapshot = populated_snapshot();
    snapshot.language_tooling_projection.status = legion_protocol::LanguageToolingStatusKind::Ready;
    snapshot.language_tooling_projection.outline = vec![LanguageOutlineSymbolProjection {
        symbol_id: "outline:answer".to_string(),
        label: "answer".to_string(),
        kind_label: "function".to_string(),
        range: Some(ProtocolTextRange {
            start: coord(6, 0, 40),
            end: coord(8, 1, 64),
        }),
        depth: 0,
        children_omitted: false,
        schema_version: 1,
    }];

    let symbols_ctx = egui::Context::default();
    symbols_ctx.enable_accesskit();
    let mut symbols_view = ProjectionView::new();
    let (_initial, full) = render_projection_frame(&symbols_ctx, &mut symbols_view, &snapshot);
    let (_selected, _full) =
        click_accessible_control(&symbols_ctx, &mut symbols_view, &snapshot, &full, "Symbols");
    let (_settled, full) = render_projection_frame(&symbols_ctx, &mut symbols_view, &snapshot);
    assert!(accesskit_has_label(&full, "answer · function · line 7"));
    for forbidden in [
        "schema",
        "projected",
        "problems=",
        "quick_fixes=",
        "stale=",
        "cancelled=",
    ] {
        assert!(
            !accesskit_contains_text_in_x_range(&full, forbidden, 46.0..=294.0),
            "Symbols must not expose internal copy `{forbidden}`"
        );
    }

    let diagnostics_bounds = accesskit_button_bounds_in_x_range(&full, "Diagnostics", 0.0..=46.0);
    let (_diagnostics, _full) = click_projection_at(
        &symbols_ctx,
        &mut symbols_view,
        &snapshot,
        egui::pos2(
            ((diagnostics_bounds.x0 + diagnostics_bounds.x1) * 0.5) as f32,
            ((diagnostics_bounds.y0 + diagnostics_bounds.y1) * 0.5) as f32,
        ),
        egui::vec2(1_440.0, 900.0),
    );
    let (_settled, full) = render_projection_frame(&symbols_ctx, &mut symbols_view, &snapshot);
    assert!(
        accesskit_contains_text_in_x_range(&full, "problems=", 294.0..=1_115.0),
        "Diagnostics must retain the raw language-tooling row"
    );
    assert!(
        accesskit_contains_text_in_x_range(&full, "requested=", 294.0..=1_115.0),
        "Diagnostics must retain raw font-fallback rows removed from Settings"
    );
    assert!(
        accesskit_contains_text_in_x_range(&full, "settings schema=", 294.0..=1_115.0),
        "Diagnostics must retain the settings schema metadata removed from Settings"
    );

    for (utility, dialog) in [("Setup", "Welcome to Legion"), ("Settings", "Settings")] {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut view = ProjectionView::new();
        let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
        let (_opened, _full) = click_accessible_control(&ctx, &mut view, &snapshot, &full, utility);
        let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
        let text = accesskit_dialog_text(&full, dialog);
        for forbidden in [
            "schema",
            "projected",
            "problems=",
            "quick_fixes=",
            "stale=",
            "cancelled=",
        ] {
            assert!(
                !text
                    .iter()
                    .any(|row| row.to_ascii_lowercase().contains(forbidden)),
                "{utility} must not expose internal copy `{forbidden}`; text={text:?}"
            );
        }
        if utility == "Settings" {
            let (_privacy, _full) =
                click_accessible_control(&ctx, &mut view, &snapshot, &full, "Privacy");
            let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
            let privacy_text = accesskit_dialog_text(&full, dialog);
            assert!(
                !privacy_text
                    .iter()
                    .flat_map(|row| row.split_whitespace())
                    .any(|word| word.contains('=')),
                "Settings sections must not expose raw key=value diagnostics; text={privacy_text:?}"
            );
        }
    }
}

#[test]
fn projection_rendering_persisted_wide_inspector_preserves_standard_editor_minimum() {
    let ctx = egui::Context::default();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Assist;
    let size = egui::vec2(1_184.0, 720.0);
    let _ = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    ctx.data_mut(|data| {
        data.insert_persisted(
            egui::Id::new("legion_desktop_trust"),
            egui::PanelState {
                rect: egui::Rect::from_min_size(
                    egui::pos2(size.x - 470.0, 70.0),
                    egui::vec2(470.0, 600.0),
                ),
            },
        );
    });

    for _ in 0..2 {
        let _ = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    }
    let rects = view
        .last_shell_panel_rects()
        .expect("standard shell should record panel allocations");
    let editor = view
        .last_editor_rect()
        .expect("standard shell should record the editor allocation");
    assert!(
        rects.center.width() >= 560.0 && editor.width() >= 560.0,
        "a persisted 470px inspector must be clamped before starving the standard editor; rects={rects:?}, editor={editor:?}"
    );
}

#[test]
fn projection_rendering_reopened_settings_focuses_the_selected_editor_section() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = populated_snapshot();

    let (_initial, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (_opened, _full) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Settings");
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (_editor, _full) = click_accessible_control(&ctx, &mut view, &snapshot, &full, "Editor");
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (_closed, _full) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Close Settings");
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);
    let (_reopened, _full) =
        click_accessible_control(&ctx, &mut view, &snapshot, &full, "Settings");
    let (_settled, full) = render_projection_frame(&ctx, &mut view, &snapshot);

    assert_eq!(
        accesskit_focused_label(&full),
        Some("Editor"),
        "reopening Settings must focus the section that remains selected"
    );
}

#[test]
fn projection_rendering_setup_review_focuses_privacy_and_escape_restores_setup_trigger() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let snapshot = populated_snapshot();
    let size = egui::vec2(1_440.0, 900.0);

    let (_initial, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    let setup_node = full
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("utility rail should expose AccessKit")
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.label() == Some("Setup")
                && node.supports_action(egui::accesskit::Action::Focus)
                && node.bounds().is_some_and(|bounds| bounds.x1 <= 46.0))
            .then_some(*id)
        })
        .expect("Setup utility should be focusable");
    let focus_input = desktop_raw_input_at(
        size,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Focus,
                target_tree: egui::accesskit::TreeId::ROOT,
                target_node: setup_node,
                data: None,
            },
        )],
    );
    let _ = ctx.run_ui(focus_input, |ui| {
        let _ = view.render(ui, &snapshot);
    });
    let setup_trigger_focus = ctx
        .memory(|memory| memory.focused())
        .expect("Setup trigger should accept focus");

    let (_opened, _full) =
        click_accessible_control_at(&ctx, &mut view, &snapshot, &full, "Setup", size);
    let (_settled, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    let (_review, _full) =
        click_accessible_control_at(&ctx, &mut view, &snapshot, &full, "Review Settings", size);
    let (_settled, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    assert_eq!(accesskit_focused_label(&full), Some("Privacy"));

    let escape = desktop_raw_input_at(
        size,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: Some(egui::Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
    );
    let _ = ctx.run_ui(escape, |ui| {
        let _ = view.render(ui, &snapshot);
    });
    let (_restored, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    assert!(!accesskit_has_role(&full, egui::accesskit::Role::Dialog));
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        Some(setup_trigger_focus)
    );
}

#[test]
fn projection_rendering_compact_inspector_drawer_resize_clamps_to_inspector_bounds() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let mut view = ProjectionView::new();
    let mut snapshot = populated_snapshot();
    snapshot.product_mode = DockMode::Assist;
    let size = egui::vec2(960.0, 720.0);
    let (_initial, full) = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    let (_opened, _full) =
        click_accessible_control_at(&ctx, &mut view, &snapshot, &full, "Inspector drawer", size);
    let _ = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    let drawer_id = egui::Id::new(("legion_desktop_compact_drawer", "Inspector", "Assist"));
    let initial = ctx
        .memory(|memory| memory.area_rect(drawer_id))
        .expect("compact inspector drawer should have a window rectangle");

    drag_projection_at(
        &ctx,
        &mut view,
        &snapshot,
        initial.right_bottom() - egui::vec2(2.0, 2.0),
        initial.right_bottom() + egui::vec2(260.0, 0.0),
        size,
    );
    let _ = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    let wide = ctx
        .memory(|memory| memory.area_rect(drawer_id))
        .expect("resized compact inspector should retain a window rectangle");
    assert!(
        wide.width() <= 482.0,
        "compact inspector must clamp expansion to 480px (plus separator tolerance); rect={wide:?}"
    );

    drag_projection_at(
        &ctx,
        &mut view,
        &snapshot,
        wide.right_bottom() - egui::vec2(2.0, 2.0),
        wide.right_bottom() - egui::vec2(360.0, 0.0),
        size,
    );
    let _ = render_projection_frame_at(&ctx, &mut view, &snapshot, size);
    let narrow = ctx
        .memory(|memory| memory.area_rect(drawer_id))
        .expect("narrow compact inspector should retain a window rectangle");
    assert!(
        narrow.width() >= 288.0,
        "compact inspector must clamp contraction to 288px; rect={narrow:?}"
    );
}

#[test]
fn projection_rendering_covers_the_four_mode_state_matrix_at_standard_and_compact_sizes() {
    let modes = [
        DockMode::Manual,
        DockMode::Assist,
        DockMode::Delegate,
        DockMode::Automate,
    ];
    let states = [
        UiStateMatrixState::Empty,
        UiStateMatrixState::Blocked,
        UiStateMatrixState::Ready,
        UiStateMatrixState::Active,
    ];
    let layouts = [
        ("standard", egui::vec2(1_440.0, 900.0)),
        ("compact", egui::vec2(960.0, 720.0)),
    ];

    for mode in modes {
        for matrix_state in states {
            for (layout, size) in layouts {
                let (snapshot, view_state, expectation) = state_matrix_case(mode, matrix_state);
                let ctx = egui::Context::default();
                ctx.enable_accesskit();
                if mode == DockMode::Delegate && matrix_state == UiStateMatrixState::Ready {
                    seed_delegate_task_draft(&ctx, "Run the focused state-matrix checks");
                }
                let mut view = ProjectionView::new();
                let (_initial, full) = render_projection_frame_with_state_at(
                    &ctx,
                    &mut view,
                    &snapshot,
                    &view_state,
                    size,
                );
                let update = full
                    .platform_output
                    .accesskit_update
                    .as_ref()
                    .unwrap_or_else(|| {
                        panic!("{mode:?}/{matrix_state:?}/{layout} must expose AccessKit")
                    });
                let mode_node = update
                    .nodes
                    .iter()
                    .find_map(|(_id, node)| {
                        (node.label() == Some(mode.label())
                            && node.role() == egui::accesskit::Role::Button
                            && node.bounds().is_some_and(|bounds| bounds.y1 <= 42.0))
                        .then_some(node)
                    })
                    .unwrap_or_else(|| {
                        panic!("{mode:?}/{matrix_state:?}/{layout} must retain its mode control")
                    });
                assert_eq!(mode_node.is_selected(), Some(true));
                assert_eq!(
                    mode_node.aria_current(),
                    Some(egui::accesskit::AriaCurrent::True)
                );

                let editor = view
                    .last_editor_rect()
                    .expect("every matrix frame must retain the editor")
                    .intersect(egui::Rect::from_min_size(egui::Pos2::ZERO, size));
                assert!(
                    editor.width() >= 360.0 && editor.height() >= 180.0,
                    "{mode:?}/{matrix_state:?}/{layout} must preserve a usable editor; editor={editor:?}"
                );

                let (visible, inspector_bounds) = if layout == "compact" && mode != DockMode::Manual
                {
                    assert!(
                        accesskit_has_clickable_label(&full, "Inspector drawer"),
                        "{mode:?}/{matrix_state:?} must keep its compact inspector reachable"
                    );
                    let (_opened, _full) = click_accessible_control_with_state_at(
                        &ctx,
                        &mut view,
                        &snapshot,
                        &view_state,
                        &full,
                        "Inspector drawer",
                        size,
                    );
                    let visible = render_projection_frame_with_state_at(
                        &ctx,
                        &mut view,
                        &snapshot,
                        &view_state,
                        size,
                    )
                    .1;
                    let inspector_bounds = accesskit_largest_label_bounds(&visible, "Inspector")
                        .unwrap_or_else(|| {
                            panic!(
                                "{mode:?}/{matrix_state:?} compact drawer must expose Inspector bounds"
                            )
                        });
                    (visible, Some(inspector_bounds))
                } else {
                    (full, None)
                };
                match expectation {
                    UiStateMatrixExpectation::Text(expected_label) => {
                        let present = inspector_bounds.map_or_else(
                            || accesskit_contains_text(&visible, expected_label),
                            |bounds| {
                                accesskit_contains_text_in_bounds(&visible, expected_label, bounds)
                            },
                        );
                        assert!(
                            present,
                            "{mode:?}/{matrix_state:?}/{layout} must expose `{expected_label}` in its state surface"
                        );
                    }
                    UiStateMatrixExpectation::Clickable(expected_label) => {
                        let operable = inspector_bounds.map_or_else(
                            || accesskit_has_clickable_label(&visible, expected_label),
                            |bounds| {
                                accesskit_clickable_label_in_bounds(
                                    &visible,
                                    expected_label,
                                    bounds,
                                )
                            },
                        );
                        assert!(
                            operable,
                            "{mode:?}/{matrix_state:?}/{layout} ready action `{expected_label}` must be operable in its state surface"
                        );
                    }
                    UiStateMatrixExpectation::Disabled { label, explanation } => {
                        assert!(
                            accesskit_label_is_disabled(&visible, label),
                            "{mode:?}/{matrix_state:?}/{layout} must expose disabled `{label}`"
                        );
                        assert!(
                            accesskit_label_has_description(&visible, label, explanation)
                                || accesskit_contains_text(&visible, explanation),
                            "{mode:?}/{matrix_state:?}/{layout} must explain why `{label}` is disabled"
                        );
                    }
                    UiStateMatrixExpectation::DirtyEditor {
                        tab_label,
                        description,
                    } => {
                        assert!(
                            snapshot.active_buffer_projection.dirty,
                            "Manual Active must be backed by a genuinely dirty buffer projection"
                        );
                        assert!(
                            accesskit_label_has_description(&visible, tab_label, description),
                            "{mode:?}/{matrix_state:?}/{layout} must expose the dirty editor state semantically"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn projection_rendering_tests_preserve_app_boundary() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/view.rs"))
        .expect("renderer source should be readable");

    common::assert_source_excludes(&source, "src/view.rs", &["legion_app", "AppComposition"]);
}
