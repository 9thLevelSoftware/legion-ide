use std::path::PathBuf;

use legion_desktop::bridge::{
    DesktopAction, DesktopAppRequest, DesktopBridgeError, DesktopBridgeOutput, DesktopCommandBridge,
};
use legion_desktop::view::DesktopProjectionViewModel;
use legion_protocol::{
    AssistedAiOperationClass, AssistedAiProjection, AssistedAiProviderAvailabilityState,
    AssistedAiProviderCapabilitySummary, AssistedAiProviderClass,
    AssistedAiProviderInvocationState, BufferId, BufferVersion, CanonicalPath,
    DebugConfigurationId, DebugSessionId, FileFingerprint, FileId, ProposalPrivacyLabel,
    ProposalRiskLabel, ProtocolTextRange, SnapshotId, TextCoordinate, TimestampMillis,
    ViewportScroll, WorkspaceId,
};
use legion_ui::ui::{DailyEditingProjection, EditorTabProjection, EditorTabsProjection};
use legion_ui::{
    ActiveBufferProjection, AssistInlinePredictionProjection, AssistInlinePredictionRowProjection,
    AssistInlinePredictionStatusProjection, CommandDispatchIntent, DebugConfigurationProjection,
    DebugStepKindProjection, ExplorerNodeProjection, ExplorerProjection, PaletteMode,
    SearchScopeProjection, Shell, ThemePreferenceProjection, ToastVerbosityProjection,
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

fn snapshot_with_active_buffer() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("Bridge").projection_snapshot();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        buffer_id: Some(BufferId(9)),
        ..ActiveBufferProjection::empty()
    };
    snapshot
}

fn snapshot_with_daily_tabs() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = snapshot_with_active_buffer();
    snapshot.daily_editing_projection = DailyEditingProjection {
        tabs: EditorTabsProjection {
            tabs: vec![
                EditorTabProjection {
                    buffer_id: BufferId(9),
                    file_id: Some(FileId(2)),
                    file_path: Some(CanonicalPath("src/lib.rs".to_string())),
                    title: "lib.rs".to_string(),
                    active: true,
                    dirty: false,
                    pinned: false,
                    preview: false,
                },
                EditorTabProjection {
                    buffer_id: BufferId(10),
                    file_id: Some(FileId(3)),
                    file_path: Some(CanonicalPath("src/main.rs".to_string())),
                    title: "main.rs".to_string(),
                    active: false,
                    dirty: true,
                    pinned: false,
                    preview: false,
                },
            ],
            active_buffer_id: Some(BufferId(9)),
        },
        ..DailyEditingProjection::empty()
    };
    snapshot.explorer_projection = ExplorerProjection {
        nodes: vec![
            ExplorerNodeProjection {
                file_id: FileId(2),
                canonical_path: CanonicalPath("src".to_string()),
                name: "src".to_string(),
                children: vec![FileId(3)],
            },
            ExplorerNodeProjection {
                file_id: FileId(3),
                canonical_path: CanonicalPath("src/main.rs".to_string()),
                name: "main.rs".to_string(),
                children: Vec::new(),
            },
        ],
        selection: None,
    };
    snapshot
}

fn snapshot_without_active_buffer() -> legion_ui::ShellProjectionSnapshot {
    Shell::empty("Bridge").projection_snapshot()
}

fn snapshot_with_debug_projection() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = snapshot_with_daily_tabs();
    snapshot.debug_projection.active_session_id = Some(DebugSessionId("debug-1".to_string()));
    snapshot
        .debug_projection
        .configurations
        .push(DebugConfigurationProjection {
            configuration_id: DebugConfigurationId("cargo:sample:bin:sample".to_string()),
            name: "Debug sample".to_string(),
            adapter_type: "lldb-dap".to_string(),
            program_label: "target/debug/sample".to_string(),
            cargo_package: Some("sample".to_string()),
            cargo_target: Some("sample".to_string()),
            deterministic: true,
        });
    snapshot
}

fn snapshot_with_assist_inline_prediction() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = snapshot_with_active_buffer();
    snapshot.active_buffer_projection.workspace_id = Some(WorkspaceId(1));
    snapshot.active_buffer_projection.file_id = Some(FileId(4));
    snapshot.assist_inline_prediction_projection = AssistInlinePredictionProjection {
        active_prediction: Some(AssistInlinePredictionRowProjection {
            prediction_id: "assist:prediction:1".to_string(),
            workspace_id: Some(WorkspaceId(1)),
            buffer_id: Some(BufferId(9)),
            file_id: Some(FileId(4)),
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
            stale: false,
            stale_reason_label: None,
            ghost_text_label: ".await".to_string(),
            replacement_preview_label: Some("future.await".to_string()),
            apply_range: range(4, 4),
            apply_range_label: "0:4..0:4".to_string(),
            diagnostics: Vec::new(),
        }),
        rows: Vec::new(),
        request_in_flight: false,
        stale_prediction_count: 0,
        after_edit_prediction_attempts: 0,
        after_edit_prediction_accepts: 0,
        generated_at: TimestampMillis(150),
        schema_version: 1,
    };
    snapshot
}

fn snapshot_with_two_assisted_ai_providers() -> legion_ui::ShellProjectionSnapshot {
    let mut snapshot = snapshot_with_active_buffer();
    snapshot.active_buffer_projection.workspace_id = Some(WorkspaceId(7));
    snapshot.active_buffer_projection.file_id = Some(FileId(11));
    snapshot.context_manifest_projection.selected_item_id = Some("context:item:7".to_string());
    snapshot.assisted_ai_projection = AssistedAiProjection {
        projection_id: "assist:projection:1".to_string(),
        providers: vec![
            AssistedAiProviderCapabilitySummary {
                provider_id: "local-mini".to_string(),
                provider_label: "Local Mini".to_string(),
                provider_class: AssistedAiProviderClass::Local,
                supported_operations: vec![
                    AssistedAiOperationClass::Explain,
                    AssistedAiOperationClass::ProposeEdit,
                ],
                supported_operation_count: 2,
                model_capability_label_count: 1,
                tool_capability_label_count: 1,
                context_window_label: "16k".to_string(),
                cost_budget_label: "$".to_string(),
                risk_budget_label: "Low".to_string(),
                privacy_retention_label: "Local-only".to_string(),
                availability: AssistedAiProviderAvailabilityState::Available,
                refusal: None,
                risk_label: ProposalRiskLabel::Low,
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                redaction_hints: Vec::new(),
                schema_version: 1,
            },
            AssistedAiProviderCapabilitySummary {
                provider_id: "remote-pro".to_string(),
                provider_label: "Remote Pro".to_string(),
                provider_class: AssistedAiProviderClass::HostedRemote,
                supported_operations: vec![
                    AssistedAiOperationClass::Explain,
                    AssistedAiOperationClass::ProposeEdit,
                    AssistedAiOperationClass::StructuredMetadata,
                ],
                supported_operation_count: 3,
                model_capability_label_count: 2,
                tool_capability_label_count: 2,
                context_window_label: "128k".to_string(),
                cost_budget_label: "$$$".to_string(),
                risk_budget_label: "Medium".to_string(),
                privacy_retention_label: "Redacted".to_string(),
                availability: AssistedAiProviderAvailabilityState::Available,
                refusal: None,
                risk_label: ProposalRiskLabel::Medium,
                privacy_label: ProposalPrivacyLabel::WorkspaceMetadata,
                redaction_hints: Vec::new(),
                schema_version: 1,
            },
        ],
        routes: Vec::new(),
        requests: Vec::new(),
        refusals: Vec::new(),
        proposal_previews: Vec::new(),
        provider_count: 2,
        request_count: 0,
        refusal_count: 0,
        preview_ready_count: 0,
        provider_invocation: AssistedAiProviderInvocationState::NotEncoded,
        generated_at: TimestampMillis(200),
        redaction_hints: Vec::new(),
        schema_version: 1,
    };
    snapshot
}

#[test]
fn assistant_rows_show_two_providers_and_selection_context() {
    let model =
        DesktopProjectionViewModel::from_snapshot(&snapshot_with_two_assisted_ai_providers());

    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("assisted ai: 2 providers")
            && row.contains("0 requests")
            && row.contains("0 refusals")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("assisted provider local-mini: Local Mini")
            && row.contains("class=Local")
            && row.contains("ops=2")
            && row.contains("privacy=Local-only")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("assisted provider remote-pro: Remote Pro")
            && row.contains("class=HostedRemote")
            && row.contains("ops=3")
            && row.contains("privacy=Redacted")
    }));
    assert!(model.assistant_rows.iter().any(|row| {
        row.contains("context manifest") && row.contains("selected=context:item:7")
    }));
}

fn translate(action: DesktopAction) -> DesktopBridgeOutput {
    DesktopCommandBridge::new().translate(action, &snapshot_with_active_buffer())
}

#[test]
fn intent_bridge_routes_save_edit_undo_redo_actions() {
    let insert_at = coord(0, 2, 2);
    assert_eq!(
        translate(DesktopAction::SaveActive),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Save {
            buffer_id: BufferId(9)
        })
    );
    assert_eq!(
        translate(DesktopAction::InsertText {
            text: "hello".to_string(),
            at: insert_at,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Insert {
            buffer_id: BufferId(9),
            at: insert_at,
            text: "hello".to_string(),
        })
    );
    assert_eq!(
        translate(DesktopAction::ReplaceRange {
            range: range(0, 5),
            replacement: "world".to_string(),
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Replace {
            buffer_id: BufferId(9),
            range: range(0, 5),
            replacement: "world".to_string(),
        })
    );
    assert_eq!(
        translate(DesktopAction::DeleteRange { range: range(1, 3) }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Delete {
            buffer_id: BufferId(9),
            range: range(1, 3),
        })
    );
    assert_eq!(
        translate(DesktopAction::Undo),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Undo {
            buffer_id: BufferId(9)
        })
    );
    assert_eq!(
        translate(DesktopAction::Redo),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Redo {
            buffer_id: BufferId(9)
        })
    );
}

#[test]
fn intent_bridge_routes_daily_editing_actions() {
    let snapshot = snapshot_with_daily_tabs();
    let bridge = DesktopCommandBridge::new();
    let cursor = coord(3, 4, 12);
    let scroll = ViewportScroll {
        top_line: 8,
        left_column: 2,
    };

    assert_eq!(
        bridge.translate(DesktopAction::SaveAll, &snapshot),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SaveAll)
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::SwitchTab {
                buffer_id: BufferId(10)
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SwitchTab {
            buffer_id: BufferId(10)
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::CloseTab {
                buffer_id: BufferId(10)
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::CloseTab {
            buffer_id: BufferId(10)
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::SetCursor {
                buffer_id: None,
                cursor,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetCursor {
            buffer_id: BufferId(9),
            cursor,
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::SetSelection {
                buffer_id: Some(BufferId(10)),
                range: range(1, 4),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetSelection {
            buffer_id: BufferId(10),
            range: range(1, 4),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::SetViewportScroll {
                buffer_id: Some(BufferId(10)),
                scroll,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetViewportScroll {
            buffer_id: BufferId(10),
            scroll,
        })
    );
}

#[test]
fn intent_bridge_routes_explorer_actions_and_adapter_local_toggle() {
    let snapshot = snapshot_with_daily_tabs();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(
            DesktopAction::ToggleExplorerPath {
                path: " src ".to_string()
            },
            &snapshot,
        ),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::ToggleExplorerPath {
            path: "src".to_string()
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::SelectExplorerFile { file_id: FileId(3) },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RevealInExplorer { file_id: FileId(3) })
    );
    assert_eq!(
        bridge.translate(DesktopAction::RefreshGit, &snapshot),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RefreshGit)
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::StageGitHunk {
                hunk_id: "git-hunk:1".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::StageGitHunk {
            hunk_id: "git-hunk:1".to_string(),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::UnstageGitHunk {
                hunk_id: "git-hunk:1".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::UnstageGitHunk {
            hunk_id: "git-hunk:1".to_string(),
        })
    );
}

#[test]
fn intent_bridge_routes_search_actions() {
    assert_eq!(
        translate(DesktopAction::OpenPalette {
            mode: PaletteMode::Search,
            query: "/".to_string(),
            scope: SearchScopeProjection::Workspace,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenPalette {
            mode: PaletteMode::Search,
            query: "/".to_string(),
            scope: SearchScopeProjection::Workspace,
        })
    );
    assert_eq!(
        translate(DesktopAction::OpenPalette {
            mode: PaletteMode::StructuralSearch,
            query: "#".to_string(),
            scope: SearchScopeProjection::Workspace,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenPalette {
            mode: PaletteMode::StructuralSearch,
            query: "#".to_string(),
            scope: SearchScopeProjection::Workspace,
        })
    );
    assert_eq!(
        translate(DesktopAction::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "needle".to_string(),
            limit: 7,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "needle".to_string(),
            limit: 7,
        })
    );
    assert_eq!(
        translate(DesktopAction::RunStructuralSearch {
            scope: SearchScopeProjection::Workspace,
            pattern: "fn $NAME ( )".to_string(),
            rewrite: Some("fn renamed_$NAME ( )".to_string()),
            limit: 11,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RunStructuralSearch {
            scope: SearchScopeProjection::Workspace,
            pattern: "fn $NAME ( )".to_string(),
            rewrite: Some("fn renamed_$NAME ( )".to_string()),
            limit: 11,
        })
    );
    assert_eq!(
        translate(DesktopAction::CancelSearch {
            query_id: "search:1".to_string(),
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::CancelSearch {
            query_id: "search:1".to_string(),
        })
    );
}

#[test]
fn intent_bridge_routes_debug_actions() {
    let snapshot = snapshot_with_debug_projection();
    let bridge = DesktopCommandBridge::new();
    let configuration_id = DebugConfigurationId("cargo:sample:bin:sample".to_string());
    let session_id = DebugSessionId("debug-1".to_string());
    let position = coord(12, 4, 88);

    assert_eq!(
        bridge.translate(DesktopAction::RefreshDebugConfigurations, &snapshot),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RefreshDebugConfigurations)
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::ToggleDebugBreakpoint {
                line: 12,
                condition: Some("count > 2".to_string()),
                hit_condition: Some("3".to_string()),
                log_message: Some("count changed".to_string()),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::ToggleDebugBreakpoint {
            buffer_id: BufferId(9),
            line: 12,
            condition: Some("count > 2".to_string()),
            hit_condition: Some("3".to_string()),
            log_message: Some("count changed".to_string()),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::LaunchDebugSession {
                configuration_id: configuration_id.clone(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::LaunchDebugSession { configuration_id })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::DebugStep {
                session_id: session_id.clone(),
                kind: DebugStepKindProjection::Over,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::DebugStep {
            session_id: session_id.clone(),
            kind: DebugStepKindProjection::Over,
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::DebugRunToCursor {
                session_id: session_id.clone(),
                position,
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::DebugRunToCursor {
            session_id: session_id.clone(),
            buffer_id: BufferId(9),
            position,
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::DebugEvaluateSelection {
                session_id: session_id.clone(),
                expression_label: "count".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::DebugEvaluateSelection {
            session_id: session_id.clone(),
            expression_label: "count".to_string(),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::DebugAddWatch {
                session_id: session_id.clone(),
                expression_label: "count".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::DebugAddWatch {
            session_id,
            expression_label: "count".to_string(),
        })
    );
}

#[test]
fn intent_bridge_routes_assist_rail_slash_commands_through_proposals() {
    let snapshot = snapshot_with_two_assisted_ai_providers();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(
            DesktopAction::StartAiExplain {
                instruction_label: "desktop /explain".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::StartAiExplain {
            instruction_label: "desktop /explain".to_string(),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::StartAiProposal {
                instruction_label: "desktop /fix".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::StartAiProposal {
            instruction_label: "desktop /fix".to_string(),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::StartAiProposal {
                instruction_label: "desktop /test".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::StartAiProposal {
            instruction_label: "desktop /test".to_string(),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::StartAiProposal {
                instruction_label: "desktop /doc".to_string(),
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::StartAiProposal {
            instruction_label: "desktop /doc".to_string(),
        })
    );
}

#[test]
fn intent_bridge_routes_assist_inline_prediction_actions() {
    let snapshot = snapshot_with_assist_inline_prediction();
    let bridge = DesktopCommandBridge::new();
    let position = coord(0, 4, 4);

    assert_eq!(
        bridge.translate(
            DesktopAction::RequestAssistInlinePrediction { position },
            &snapshot,
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RequestAssistInlinePrediction {
            buffer_id: BufferId(9),
            position,
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::AcceptCurrentAssistInlinePrediction,
            &snapshot
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::AcceptAssistInlinePrediction {
            buffer_id: BufferId(9),
            prediction_id: Some("assist:prediction:1".to_string()),
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::DismissCurrentAssistInlinePrediction,
            &snapshot
        ),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::DismissAssistInlinePrediction {
            buffer_id: BufferId(9),
            prediction_id: Some("assist:prediction:1".to_string()),
        })
    );
    assert_eq!(
        bridge.translate(DesktopAction::CancelAssistInlinePrediction, &snapshot),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::CancelAssistInlinePrediction {
            buffer_id: BufferId(9),
            prediction_id: Some("assist:prediction:1".to_string()),
        })
    );
}

#[test]
fn intent_bridge_preserves_clipboard_and_ime_text() {
    let at = coord(1, 0, 7);
    assert_eq!(
        translate(DesktopAction::ClipboardCopy),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::ClipboardCopy {
            buffer_id: BufferId(9),
        })
    );
    assert_eq!(
        translate(DesktopAction::ClipboardCut),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::ClipboardCut {
            buffer_id: BufferId(9),
        })
    );
    assert_eq!(
        translate(DesktopAction::ClipboardPaste {
            text: "clip ñ".to_string(),
            at,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Insert {
            buffer_id: BufferId(9),
            at,
            text: "clip ñ".to_string(),
        })
    );
    assert_eq!(
        translate(DesktopAction::ImeCommit {
            text: "入力".to_string(),
            at,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Insert {
            buffer_id: BufferId(9),
            at,
            text: "入力".to_string(),
        })
    );
    assert_eq!(
        translate(DesktopAction::SelectAll { buffer_id: None }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SelectAll {
            buffer_id: BufferId(9),
        })
    );
}

#[test]
fn intent_bridge_routes_path_dialog_prompt_refresh_and_quit() {
    assert_eq!(
        translate(DesktopAction::OpenPathText("  Cargo.toml  ".to_string())),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenPath {
            path: "Cargo.toml".to_string(),
        })
    );
    assert_eq!(
        translate(DesktopAction::OpenPathDialogSelected(
            "src/main.rs".to_string()
        )),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenPath {
            path: "src/main.rs".to_string(),
        })
    );
    assert_eq!(
        translate(DesktopAction::OpenPathDialogCancelled),
        DesktopBridgeOutput::Noop
    );
    assert_eq!(
        translate(DesktopAction::OpenPalette {
            mode: PaletteMode::File,
            query: String::new(),
            scope: SearchScopeProjection::ActiveFile,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenPalette {
            mode: PaletteMode::File,
            query: String::new(),
            scope: SearchScopeProjection::ActiveFile,
        })
    );
    assert_eq!(
        translate(DesktopAction::OpenWorkspace {
            root: PathBuf::from(".")
        }),
        DesktopBridgeOutput::AppRequest(DesktopAppRequest::OpenWorkspace {
            root: PathBuf::from(".")
        })
    );
    assert_eq!(
        translate(DesktopAction::RefreshExplorer),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RefreshExplorer)
    );
    assert_eq!(
        translate(DesktopAction::Quit),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::Quit)
    );
}

#[test]
fn intent_bridge_routes_palette_reducer_actions() {
    assert_eq!(
        translate(DesktopAction::UpdatePaletteQuery {
            query: ">save".to_string(),
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::UpdatePaletteQuery {
            query: ">save".to_string(),
        })
    );
    assert_eq!(
        translate(DesktopAction::MovePaletteSelection { delta: 1 }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::MovePaletteSelection { delta: 1 })
    );
    assert_eq!(
        translate(DesktopAction::CompletePaletteSelection),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::CompletePaletteSelection)
    );
    assert_eq!(
        translate(DesktopAction::DispatchPaletteSelection),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::DispatchPaletteSelection)
    );
    assert_eq!(
        translate(DesktopAction::ClosePalette),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::ClosePalette)
    );
}

#[test]
fn intent_bridge_routes_settings_actions() {
    assert_eq!(
        translate(DesktopAction::OpenSettings),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::OpenSettings)
    );
    assert_eq!(
        translate(DesktopAction::SetThemePreference {
            preference: ThemePreferenceProjection::System,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetThemePreference {
            preference: ThemePreferenceProjection::System,
        })
    );
    assert_eq!(
        translate(DesktopAction::SetZoomPercent { zoom_percent: 150 }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetZoomPercent { zoom_percent: 150 })
    );
    assert_eq!(
        translate(DesktopAction::SetEditorFontSize { font_size_pt: 15 }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetEditorFontSize { font_size_pt: 15 })
    );
    assert_eq!(
        translate(DesktopAction::SetToastVerbosity {
            verbosity: ToastVerbosityProjection::All,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetToastVerbosity {
            verbosity: ToastVerbosityProjection::All,
        })
    );
    assert_eq!(
        translate(DesktopAction::SetLineNumbersVisible { visible: false }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetLineNumbersVisible {
            visible: false,
        })
    );
    assert_eq!(
        translate(DesktopAction::SetCurrentLineHighlight { enabled: false }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetCurrentLineHighlight {
            enabled: false,
        })
    );
    assert_eq!(
        translate(DesktopAction::SetStickyHeadersVisible { visible: true }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetStickyHeadersVisible {
            visible: true
        })
    );
    assert_eq!(
        translate(DesktopAction::SetCodeFoldingVisible { visible: true }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetCodeFoldingVisible { visible: true })
    );
    assert_eq!(
        translate(DesktopAction::SetMinimapVisible { visible: false }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetMinimapVisible { visible: false })
    );
    assert_eq!(
        translate(DesktopAction::SetWhitespaceGuidesVisible { visible: true }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetWhitespaceGuidesVisible {
            visible: true,
        })
    );
    assert_eq!(
        translate(DesktopAction::SetIndentGuidesVisible { visible: true }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetIndentGuidesVisible {
            visible: true
        })
    );
    assert_eq!(
        translate(DesktopAction::SetSmoothScrollingEnabled { enabled: false }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetSmoothScrollingEnabled {
            enabled: false,
        })
    );
    assert_eq!(
        translate(DesktopAction::SetCrashReportsEnabled { enabled: true }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::SetCrashReportsEnabled {
            enabled: true,
        })
    );
    assert_eq!(
        translate(DesktopAction::ResetSettings),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::ResetSettings)
    );
}

#[test]
fn intent_bridge_routes_toast_actions_without_product_state_ownership() {
    assert_eq!(
        translate(DesktopAction::DismissToast { toast_id: 42 }),
        DesktopBridgeOutput::Noop
    );
    assert_eq!(
        translate(DesktopAction::InvokeToastAction {
            intent: CommandDispatchIntent::RefreshExplorer,
        }),
        DesktopBridgeOutput::Intent(CommandDispatchIntent::RefreshExplorer)
    );
}

#[test]
fn intent_bridge_rejects_invalid_path_input() {
    assert_eq!(
        translate(DesktopAction::OpenPathText(" \t ".to_string())),
        DesktopBridgeOutput::Error(DesktopBridgeError::InvalidPathInput)
    );
    assert_eq!(
        translate(DesktopAction::OpenPathDialogSelected(String::new())),
        DesktopBridgeOutput::Error(DesktopBridgeError::InvalidPathInput)
    );
}

#[test]
fn intent_bridge_rejects_buffer_actions_without_active_buffer() {
    let snapshot = snapshot_without_active_buffer();
    let bridge = DesktopCommandBridge::new();
    let actions = [
        DesktopAction::SaveActive,
        DesktopAction::InsertText {
            text: "x".to_string(),
            at: coord(0, 0, 0),
        },
        DesktopAction::ReplaceRange {
            range: range(0, 1),
            replacement: "x".to_string(),
        },
        DesktopAction::DeleteRange { range: range(0, 1) },
        DesktopAction::ClipboardCopy,
        DesktopAction::ClipboardCut,
        DesktopAction::Undo,
        DesktopAction::Redo,
        DesktopAction::SetCursor {
            buffer_id: None,
            cursor: coord(0, 0, 0),
        },
        DesktopAction::SetSelection {
            buffer_id: None,
            range: range(0, 1),
        },
        DesktopAction::SelectAll { buffer_id: None },
        DesktopAction::SetViewportScroll {
            buffer_id: None,
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
        },
    ];

    for action in actions {
        assert_eq!(
            bridge.translate(action, &snapshot),
            DesktopBridgeOutput::Error(DesktopBridgeError::MissingActiveBuffer)
        );
    }
}

#[test]
fn intent_bridge_rejects_unknown_tabs_and_explorer_files() {
    let snapshot = snapshot_with_daily_tabs();
    let bridge = DesktopCommandBridge::new();

    assert_eq!(
        bridge.translate(
            DesktopAction::SwitchTab {
                buffer_id: BufferId(99)
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownTab {
            buffer_id: BufferId(99)
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::SelectExplorerFile {
                file_id: FileId(99)
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::UnknownExplorerFile {
            file_id: FileId(99)
        })
    );
    assert_eq!(
        bridge.translate(
            DesktopAction::ToggleExplorerPath {
                path: " ".to_string()
            },
            &snapshot,
        ),
        DesktopBridgeOutput::Error(DesktopBridgeError::InvalidPathInput)
    );
}

#[test]
fn intent_bridge_preserves_app_boundary() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bridge.rs"))
        .expect("bridge source should be readable");

    common::assert_source_excludes(
        &source,
        "src/bridge.rs",
        &["AppComposition", "WorkspaceActor", "EditorEngine"],
    );
}
