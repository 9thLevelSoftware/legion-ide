use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use legion_app::{AppCommandOutcome, AppComposition, AppProductMode, AppSaveOutcome};
use legion_protocol::{
    AssistedAiProviderInvocationState, ContextManifestEgressStatus, ContextManifestItemKind,
    ContextManifestPermissionKind, DelegatedTaskProposalHunkDisposition,
    DelegatedTaskRuntimeActivationState, PrincipalId, SemanticPrivacyScope,
    TerminalPanelStatusKind, TextCoordinate, WorkspaceTrustState,
};
use legion_ui::{CommandDispatchIntent, SearchScopeProjection, SearchStatusKindProjection};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "legion_manual_zero_egress_{}_{}_{}",
            std::process::id(),
            nanos,
            id
        ));
        fs::create_dir(&root).expect("temp workspace should be created");
        fs::write(root.join("main.rs"), "fn main() {\n    let value = 1;\n}\n")
            .expect("source should be written");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn main_rs(&self) -> PathBuf {
        self.root.join("main.rs")
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let temp_root = std::env::temp_dir();
        let file_name = self.root.file_name().and_then(|name| name.to_str());
        if self.root.starts_with(&temp_root)
            && file_name.is_some_and(|name| name.starts_with("legion_manual_zero_egress_"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn coord(line: u32, character: u32, byte_offset: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character,
        byte_offset: Some(byte_offset),
        utf16_offset: Some(byte_offset),
    }
}

#[test]
fn manual_mode_open_edit_save_search_records_no_hosted_egress() {
    let workspace = TempWorkspace::new();
    let mut app = AppComposition::new();
    app.set_product_mode(AppProductMode::Manual);
    app.open_workspace(
        workspace.path(),
        WorkspaceTrustState::Trusted,
        PrincipalId("manual-smoke".to_string()),
    )
    .expect("workspace should open");
    app.open_file(workspace.main_rs().to_string_lossy())
        .expect("main.rs should open");
    let snapshot = app
        .shell_projection_snapshot("Manual")
        .expect("snapshot should project");
    let buffer_id = snapshot
        .active_buffer_projection
        .buffer_id
        .expect("active buffer should be projected");

    app.dispatch_ui_intent(CommandDispatchIntent::Insert {
        buffer_id,
        at: coord(1, 4, 16),
        text: "let local_only = true;\n    ".to_string(),
    })
    .expect("insert should route through app authority");

    let search = app
        .dispatch_ui_intent(CommandDispatchIntent::RunSearch {
            scope: SearchScopeProjection::ActiveFile,
            query: "local_only".to_string(),
            limit: 10,
            case_sensitive: None,
            whole_word: None,
            use_regex: None,
        })
        .expect("active-file search should route through app authority");
    let AppCommandOutcome::SearchUpdated(search_projection) = search else {
        panic!("expected search projection, got {search:?}");
    };
    assert_eq!(
        search_projection.status.kind,
        SearchStatusKindProjection::Completed
    );
    assert_eq!(search_projection.results.len(), 1);

    let save = app
        .dispatch_ui_intent(CommandDispatchIntent::Save { buffer_id })
        .expect("save should route through app authority");
    assert!(matches!(
        save,
        AppCommandOutcome::Save(AppSaveOutcome::Saved(_))
    ));
    assert!(
        fs::read_to_string(workspace.main_rs())
            .expect("main.rs should be readable")
            .contains("let local_only = true;")
    );

    let snapshot = app
        .shell_projection_snapshot("Manual")
        .expect("snapshot should project after save");
    assert_eq!(app.product_mode(), AppProductMode::Manual);
    assert_eq!(snapshot.product_mode, legion_ui::DockMode::Manual);
    assert_eq!(snapshot.assisted_ai_projection.provider_count, 0);
    assert_eq!(snapshot.assisted_ai_projection.request_count, 0);
    assert_eq!(snapshot.assisted_ai_projection.refusal_count, 0);
    assert_eq!(snapshot.assisted_ai_projection.preview_ready_count, 0);
    assert_eq!(
        snapshot.assisted_ai_projection.provider_invocation,
        AssistedAiProviderInvocationState::NotEncoded
    );
    assert!(snapshot.assisted_ai_projection.providers.is_empty());
    assert!(snapshot.assisted_ai_projection.routes.is_empty());
    assert!(snapshot.assisted_ai_projection.requests.is_empty());
    assert!(snapshot.assisted_ai_projection.refusals.is_empty());
    assert!(snapshot.assisted_ai_projection.proposal_previews.is_empty());
    assert!(
        snapshot
            .assist_inline_prediction_projection
            .active_prediction
            .is_none()
    );
    assert!(snapshot.assist_inline_prediction_projection.rows.is_empty());
    assert!(
        !snapshot
            .assist_inline_prediction_projection
            .request_in_flight
    );
    assert_eq!(
        snapshot
            .assist_inline_prediction_projection
            .stale_prediction_count,
        0
    );
    assert_eq!(
        snapshot
            .assist_inline_prediction_projection
            .after_edit_prediction_attempts,
        0
    );
    assert_eq!(
        snapshot
            .assist_inline_prediction_projection
            .after_edit_prediction_accepts,
        0
    );
    assert_eq!(snapshot.delegated_task_projection.plan_count, 0);
    assert_eq!(snapshot.delegated_task_projection.blocked_plan_count, 0);
    assert_eq!(snapshot.delegated_task_projection.refused_plan_count, 0);
    assert_eq!(
        snapshot.delegated_task_projection.runtime_activation,
        DelegatedTaskRuntimeActivationState::NotEncoded
    );
    assert!(snapshot.delegated_task_projection.plan_rows.is_empty());
    assert!(snapshot.delegated_task_projection.step_summaries.is_empty());
    assert!(snapshot.delegated_task_projection.blockers.is_empty());
    assert!(snapshot.delegated_task_projection.refusals.is_empty());
    assert!(
        snapshot
            .delegated_task_projection
            .required_approvals
            .is_empty()
    );
    assert!(
        snapshot
            .delegated_task_projection
            .proposal_preview_links
            .is_empty()
    );
    assert!(
        snapshot
            .delegated_task_projection
            .audit_readiness
            .is_empty()
    );
    assert!(snapshot.delegated_task_projection.chat_messages.is_empty());
    assert!(
        snapshot
            .delegated_task_projection
            .context_citations
            .is_empty()
    );
    assert!(
        snapshot
            .delegated_task_projection
            .proposal_reviews
            .iter()
            .all(|review| review.human_approval_required
                && !review.ready_for_apply
                && !review.filtered_apply_required
                && review.accepted_hunk_count == 0
                && review.rejected_hunk_count == 0
                && review.pending_hunk_count == review.hunks.len() as u32
                && review
                    .hunks
                    .iter()
                    .all(|hunk| hunk.disposition == DelegatedTaskProposalHunkDisposition::Pending))
    );
    assert!(
        snapshot
            .delegated_task_projection
            .tool_permission_requests
            .is_empty()
    );
    assert_eq!(snapshot.delegated_task_projection.chat_message_count, 0);
    assert_eq!(snapshot.delegated_task_projection.context_citation_count, 0);
    assert_eq!(
        snapshot.delegated_task_projection.proposal_review_count,
        snapshot.delegated_task_projection.proposal_reviews.len() as u32
    );
    assert_eq!(
        snapshot
            .delegated_task_projection
            .tool_permission_request_count,
        0
    );
    assert_eq!(
        snapshot.context_manifest_projection.manifest.egress,
        ContextManifestEgressStatus::LocalOnly
    );
    assert!(
        snapshot
            .context_manifest_projection
            .manifest
            .items
            .iter()
            .all(|item| item.egress == ContextManifestEgressStatus::LocalOnly
                && item
                    .privacy_scope
                    .is_none_or(|scope| scope == SemanticPrivacyScope::MetadataOnly)
                && item.kind != ContextManifestItemKind::ProviderRoute)
    );
    assert!(
        snapshot
            .context_manifest_projection
            .manifest
            .permissions
            .iter()
            .all(
                |permission| permission.egress == ContextManifestEgressStatus::LocalOnly
                    && permission.privacy_scope == SemanticPrivacyScope::MetadataOnly
                    && permission.kind != ContextManifestPermissionKind::ModelProvider
            )
    );
    assert_eq!(
        snapshot
            .context_manifest_projection
            .manifest
            .omitted_item_count,
        0
    );
    assert!(
        !snapshot
            .context_manifest_projection
            .manifest
            .stale_or_missing_metadata_risk_present
    );
    assert!(
        snapshot
            .context_manifest_projection
            .selected_item_id
            .is_none()
    );
    assert_eq!(
        snapshot
            .privacy_inspector_projection
            .external_egress_record_count,
        0
    );
    assert!(
        snapshot
            .privacy_inspector_projection
            .records
            .iter()
            .all(|record| record.egress == ContextManifestEgressStatus::LocalOnly)
    );
    assert!(!snapshot.settings_projection.telemetry.enabled);
    assert!(!snapshot.settings_projection.telemetry.crash_reports_enabled);
    assert!(!snapshot.settings_projection.telemetry.raw_source_allowed);
    assert_eq!(
        snapshot.settings_projection.telemetry.consent_label,
        "local-only"
    );
    assert!(snapshot.plugin_contribution_projections.is_empty());
    assert!(snapshot.collaboration_presence_projections.is_empty());
    assert!(!snapshot.collaboration_gui_projection.runtime_enabled);
    assert!(!snapshot.collaboration_gui_projection.presence_enabled);
    assert!(
        snapshot
            .collaboration_gui_projection
            .session_rows
            .is_empty()
    );
    assert!(
        snapshot
            .collaboration_gui_projection
            .shared_proposal_rows
            .is_empty()
    );
    assert_eq!(
        snapshot
            .collaboration_gui_projection
            .reconnecting_session_count,
        0
    );
    assert_eq!(
        snapshot.collaboration_gui_projection.conflict_session_count,
        0
    );
    assert_eq!(
        snapshot.collaboration_gui_projection.offline_session_count,
        0
    );
    assert!(!snapshot.remote_gui_projection.runtime_enabled);
    assert!(snapshot.remote_gui_projection.session_rows.is_empty());
    assert!(
        snapshot
            .remote_gui_projection
            .proposal_review_rows
            .is_empty()
    );
    assert_eq!(snapshot.remote_gui_projection.connected_session_count, 0);
    assert_eq!(snapshot.remote_gui_projection.reconnecting_session_count, 0);
    assert_eq!(snapshot.remote_gui_projection.offline_session_count, 0);
    assert!(snapshot.legion_workflow_projection.rows.is_empty());
    assert!(
        snapshot
            .legion_workflow_projection
            .mcp_registries
            .is_empty()
    );
    assert!(snapshot.legion_workflow_projection.decision_feed.is_empty());
    assert!(snapshot.legion_workflow_projection.risk_monitors.is_empty());
    assert!(snapshot.legion_workflow_projection.kill_switches.is_empty());
    assert!(
        snapshot
            .legion_workflow_projection
            .tool_permission_requests
            .is_empty()
    );
    assert_eq!(snapshot.legion_workflow_projection.total_session_count, 0);
    assert_eq!(snapshot.legion_workflow_projection.mcp_registry_count, 0);
    assert_eq!(snapshot.legion_workflow_projection.decision_feed_count, 0);
    assert_eq!(snapshot.legion_workflow_projection.risk_monitor_count, 0);
    assert_eq!(snapshot.legion_workflow_projection.kill_switch_count, 0);
    assert_eq!(
        snapshot
            .legion_workflow_projection
            .tool_permission_request_count,
        0
    );
    assert_eq!(snapshot.legion_workflow_projection.omitted_row_count, 0);
    assert!(snapshot.terminal_panel_projection.workspace_id.is_none());
    assert!(
        snapshot
            .terminal_panel_projection
            .active_session_id
            .is_none()
    );
    assert!(snapshot.terminal_panel_projection.runtime_state.is_none());
    assert_eq!(
        snapshot.terminal_panel_projection.status.kind,
        TerminalPanelStatusKind::Disabled
    );
    assert!(snapshot.terminal_panel_projection.policy.is_none());
    assert!(snapshot.terminal_panel_projection.output_rows.is_empty());
    assert_eq!(
        snapshot
            .terminal_panel_projection
            .scrollback
            .visible_row_count,
        0
    );
    assert_eq!(
        snapshot
            .terminal_panel_projection
            .scrollback
            .omitted_row_count,
        0
    );
    assert!(!snapshot.terminal_panel_projection.scrollback.truncated);
    assert_eq!(snapshot.terminal_panel_projection.search.match_count, 0);
    assert!(
        snapshot
            .terminal_panel_projection
            .search
            .query_label
            .is_none()
    );
    assert!(snapshot.terminal_panel_projection.last_error.is_none());
    assert!(snapshot.terminal_panel_projection.last_denial.is_none());
    assert!(
        snapshot
            .status_messages
            .iter()
            .all(|status| !status.message.to_ascii_lowercase().contains("http"))
    );
}
